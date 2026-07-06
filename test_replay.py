"""Replay the exact model calls against LanceDB to reproduce the bug."""

import lancedb
import os
import time

uri = os.path.join(os.environ['APPDATA'], r'com.squirecli.app\squire_lancedb')
db = lancedb.connect(uri)

# First, create a fresh test table to avoid polluting real data
# Actually, let's use a temp directory
import tempfile
import shutil

tmpdir = tempfile.mkdtemp()
print(f"Using temp dir: {tmpdir}")
db2 = lancedb.connect(tmpdir)

# Create tables with same schemas as Rust code
import pyarrow as pa

# Create tokens table
tokens_schema = pa.schema([
    pa.field("token_id", pa.utf8(), nullable=False),
    pa.field("token_type", pa.utf8(), nullable=False),
    pa.field("short_desc", pa.utf8(), nullable=False),
    pa.field("full_desc", pa.utf8(), nullable=True),
    pa.field("creation_turn", pa.uint64(), nullable=False),
    pa.field("accumulated_hits", pa.uint64(), nullable=False),
    pa.field("embedding", pa.list_(pa.float32(), 384), nullable=True),
    pa.field("endpoint", pa.utf8(), nullable=True),
])
db2.create_table("squire_tokens", schema=tokens_schema, exist_ok=True)

# Create relationships table
rels_schema = pa.schema([
    pa.field("subject", pa.utf8(), nullable=False),
    pa.field("predicate", pa.utf8(), nullable=False),
    pa.field("object", pa.utf8(), nullable=False),
])
db2.create_table("squire_relationships", schema=rels_schema, exist_ok=True)

rel_tbl = db2.open_table("squire_relationships")
tok_tbl = db2.open_table("squire_tokens")

print("=== Phase 1: Create root ===")
# Model call: {"operation":"create","title":"Download books from website and build local HTML reader"}
import pyarrow as pa

# upsert_token equivalent: delete + insert
root_id = "TODO_Download_books_from_website_and_build_local_HTML_reader"
table = db2.open_table("squire_tokens")
table.delete(f"token_id = '{root_id}'")
table.add([{
    "token_id": root_id,
    "token_type": "todo",
    "short_desc": "Download books from website and build local HTML reader",
    "full_desc": None,
    "creation_turn": 1,
    "accumulated_hits": 1,
    "embedding": [0.0] * 384,
    "endpoint": None,
}])
print(f"Created root: {root_id}")

print("\n=== Phase 2: Bulk create children with parent_id ===")
children_data = [
    ("TODO_Specify_the_target_website_and_book_selection_criteria", "Specify the target website and book selection criteria"),
    ("TODO_Analyze_website_structure_and_identify_book_content_URLs", "Analyze website structure and identify book content URLs"),
    ("TODO_Download_book_content_HTMLtext", "Download book content (HTML/text)"),
    ("TODO_Generate_local_HTML_index_page_for_reading", "Generate local HTML index page for reading"),
    ("TODO_Verify_local_reading_experience_works", "Verify local reading experience works"),
]

for child_id, child_title in children_data:
    # Create token
    table = db2.open_table("squire_tokens")
    table.delete(f"token_id = '{child_id}'")
    table.add([{
        "token_id": child_id,
        "token_type": "todo",
        "short_desc": child_title,
        "full_desc": None,
        "creation_turn": 1,
        "accumulated_hits": 1,
        "embedding": [0.0] * 384,
        "endpoint": None,
    }])
    
    # Insert relationship
    rel_tbl = db2.open_table("squire_relationships")
    rel_tbl.add([{
        "subject": root_id,
        "predicate": "subtask",
        "object": child_id,
    }])
    print(f"  Created child '{child_title}' with parent_id={root_id}")

print("\n=== Phase 3: List (replay the list call) ===")
# The model called list after creating
# Let's read relationships and tokens
rel_tbl = db2.open_table("squire_relationships")
print(f"Relationships: {rel_tbl.count_rows()} rows")
ds = rel_tbl.to_lance()
reader = ds.scanner().to_reader()
for batch in reader:
    for i in range(batch.num_rows):
        s = batch.column('subject')[i].as_py()
        p = batch.column('predicate')[i].as_py()
        o = batch.column('object')[i].as_py()
        print(f"  {s} -[{p}]-> {o}")

# Now simulate what load_todo_indices does:
# 1. get_relationships(None,None,None) - all rels
all_rels = []
for batch in reader:
    for i in range(batch.num_rows):
        all_rels.append((
            batch.column('subject')[i].as_py(),
            batch.column('predicate')[i].as_py(),
            batch.column('object')[i].as_py(),
        ))
# Re-open reader since we consumed it
ds = rel_tbl.to_lance()
reader = ds.scanner().to_reader()
all_rels = []
for batch in reader:
    for i in range(batch.num_rows):
        all_rels.append((
            batch.column('subject')[i].as_py(),
            batch.column('predicate')[i].as_py(),
            batch.column('object')[i].as_py(),
        ))

# 2. list_token_ids - all token IDs
tok_tbl = db2.open_table("squire_tokens")
ds2 = tok_tbl.to_lance()
reader2 = ds2.scanner().to_reader()
all_ids = []
for batch in reader2:
    for i in range(batch.num_rows):
        all_ids.append(batch.column('token_id')[i].as_py())

# 3. Build indices (exact same logic as load_todo_indices)
from collections import defaultdict
outgoing_subtask = defaultdict(list)
incoming_subtask = defaultdict(list)
marked_done = set()

for s, p, o in all_rels:
    if p == "subtask":
        outgoing_subtask[s].append(o)
        incoming_subtask[o].append(s)

todo_ids = [tid for tid in all_ids if tid.startswith("TODO_")]
root_ids = [tid for tid in todo_ids if tid not in incoming_subtask]

print(f"\nAll token IDs: {len(all_ids)}")
print(f"TODO token IDs: {len(todo_ids)}")
print(f"Root IDs: {root_ids}")
print(f"Outgoing subtask keys: {list(outgoing_subtask.keys())}")
print(f"Root {root_id} children: {outgoing_subtask.get(root_id, [])}")

# Verify: same as Rust's build_tree_json logic
for rid in root_ids:
    children = outgoing_subtask.get(rid, [])
    print(f"\nRoot '{rid}' has {len(children)} children:")
    for c in children:
        print(f"  - {c}")

# OK that's the first session. Now replay the second session's create with -2
print("\n\n=== Phase 4: Second session - create with -2 suffix ===")
root_id_2 = "TODO_Download_books_from_website_and_build_local_HTML_reader-2"
table = db2.open_table("squire_tokens")
table.delete(f"token_id = '{root_id_2}'")
table.add([{
    "token_id": root_id_2,
    "token_type": "todo",
    "short_desc": "Download books from website and build local HTML reader",
    "full_desc": None,
    "creation_turn": 2,
    "accumulated_hits": 1,
    "embedding": [0.0] * 384,
    "endpoint": None,
}])
print(f"Created root: {root_id_2}")

children_data_2 = [
    ("TODO_Phase_1_Analyze_website_structure", "Phase 1: Analyze website structure"),
    ("TODO_Phase_2_Download_book_content-2", "Phase 2: Download book content"),
    ("TODO_Phase_3_Build_local_HTML_reader-2", "Phase 3: Build local HTML reader"),
    ("TODO_Phase_4_Verify_offline_reading", "Phase 4: Verify offline reading"),
]

for child_id, child_title in children_data_2:
    table = db2.open_table("squire_tokens")
    table.delete(f"token_id = '{child_id}'")
    table.add([{
        "token_id": child_id,
        "token_type": "todo",
        "short_desc": child_title,
        "full_desc": None,
        "creation_turn": 2,
        "accumulated_hits": 1,
        "embedding": [0.0] * 384,
        "endpoint": None,
    }])
    
    rel_tbl = db2.open_table("squire_relationships")
    rel_tbl.add([{
        "subject": root_id_2,
        "predicate": "subtask",
        "object": child_id,
    }])
    print(f"  Created child '{child_title}'")

print("\n=== Phase 5: List after second session ===")
rel_tbl = db2.open_table("squire_relationships")
print(f"Relationships: {rel_tbl.count_rows()} rows")
ds = rel_tbl.to_lance()
reader = ds.scanner().to_reader()
all_rels = []
for batch in reader:
    for i in range(batch.num_rows):
        all_rels.append((
            batch.column('subject')[i].as_py(),
            batch.column('predicate')[i].as_py(),
            batch.column('object')[i].as_py(),
        ))

tok_tbl = db2.open_table("squire_tokens")
ds2 = tok_tbl.to_lance()
reader2 = ds2.scanner().to_reader()
all_ids = []
for batch in reader2:
    for i in range(batch.num_rows):
        all_ids.append(batch.column('token_id')[i].as_py())

outgoing_subtask = defaultdict(list)
incoming_subtask = defaultdict(list)
for s, p, o in all_rels:
    if p == "subtask":
        outgoing_subtask[s].append(o)
        incoming_subtask[o].append(s)

todo_ids = [tid for tid in all_ids if tid.startswith("TODO_")]
root_ids = [tid for tid in todo_ids if tid not in incoming_subtask]

print(f"Root IDs: {root_ids}")
for rid in root_ids:
    children = outgoing_subtask.get(rid, [])
    print(f"Root '{rid}' has {len(children)} children")
    for c in children:
        print(f"  - {c}")

# Phase 6: check token_detail equivalent
print("\n=== Phase 6: Get specific root ===")
print(f"Root {root_id_2} children: {outgoing_subtask.get(root_id_2, [])}")

# Cleanup
shutil.rmtree(tmpdir)
print("\n=== DONE ===")
