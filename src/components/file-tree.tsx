import { useState, useEffect, useCallback } from 'react'
import { ChevronRight, ChevronDown, File, Folder, FolderOpen, FilePlus, FolderPlus, Pencil, Trash2, ExternalLink } from 'lucide-react'
import { listDirectory, deleteItem, renameItem, createDir, gitStatus, writeFile } from '@/lib/ipc'
import { useEditorStore } from '@/stores/editor-store'
import { useLayoutStore } from '@/stores/ui-store'
import type { FileEntry } from '@/types/ipc'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu'

interface TreeNode {
  entry: FileEntry
  children: TreeNode[]
  expanded: boolean
  gitStatus?: string
}

function getIcon(node: TreeNode) {
  if (!node.entry.isDir) return <File className="h-4 w-4 shrink-0" />
  return node.expanded
    ? <FolderOpen className="h-4 w-4 shrink-0 text-[#4A90D9]" />
    : <Folder className="h-4 w-4 shrink-0 text-[#4A90D9]" />
}

function getStatusDot(status?: string) {
  if (!status) return null
  const colors: Record<string, string> = {
    modified: 'bg-yellow-400',
    added: 'bg-green-400',
    deleted: 'bg-red-400',
    renamed: 'bg-blue-400',
    staged: 'bg-yellow-500',
  }
  return (
    <span className={`w-1.5 h-1.5 rounded-full ${colors[status] ?? 'bg-gray-400'} shrink-0`} />
  )
}

function TreeItem({
  node,
  depth,
  onToggle,
  onSelect,
  onRename,
  onDelete,
  onNewFile,
  onNewFolder,
}: {
  node: TreeNode
  depth: number
  onToggle: (path: string) => void
  onSelect: (entry: FileEntry) => void
  onRename: (entry: FileEntry) => void
  onDelete: (entry: FileEntry) => void
  onNewFile: (parent: string) => void
  onNewFolder: (parent: string) => void
}) {
  const hasChildren = node.entry.isDir

  return (
    <ContextMenu>
      <ContextMenuTrigger>
        <div
          className="flex items-center gap-1 px-2 py-0.5 text-sm cursor-pointer rounded hover:bg-[#D0DCE8] group"
          style={{ paddingLeft: `${depth * 16 + 8}px` }}
          onClick={() => {
            if (hasChildren) onToggle(node.entry.path)
            else onSelect(node.entry)
          }}
        >
          {hasChildren ? (
            <span className="w-4 h-4 flex items-center justify-center shrink-0">
              {node.expanded
                ? <ChevronDown className="h-3 w-3" />
                : <ChevronRight className="h-3 w-3" />}
            </span>
          ) : (
            <span className="w-4 shrink-0" />
          )}
          {getIcon(node)}
          <span className="truncate ml-1 flex-1">{node.entry.name}</span>
          {getStatusDot(node.gitStatus)}
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent className="w-48">
        {hasChildren && (
          <>
            <ContextMenuItem onClick={() => onNewFile(node.entry.path)}>
              <FilePlus className="h-4 w-4 mr-2" />
              New File
            </ContextMenuItem>
            <ContextMenuItem onClick={() => onNewFolder(node.entry.path)}>
              <FolderPlus className="h-4 w-4 mr-2" />
              New Folder
            </ContextMenuItem>
            <ContextMenuSeparator />
          </>
        )}
        <ContextMenuItem onClick={() => onRename(node.entry)}>
          <Pencil className="h-4 w-4 mr-2" />
          Rename
        </ContextMenuItem>
        <ContextMenuItem onClick={() => onDelete(node.entry)}>
          <Trash2 className="h-4 w-4 mr-2 text-red-500" />
          <span className="text-red-500">Delete</span>
        </ContextMenuItem>
        <ContextMenuSeparator />
        <ContextMenuItem onClick={() => {/* reveal in explorer */}}>
          <ExternalLink className="h-4 w-4 mr-2" />
          Reveal in Explorer
        </ContextMenuItem>
      </ContextMenuContent>
    </ContextMenu>
  )
}

export function FileTree() {
  const [rootNodes, setRootNodes] = useState<TreeNode[]>([])
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())
  const [gitStatusMap, setGitStatusMap] = useState<Record<string, string>>({})
  const openFile = useEditorStore((s) => s.openFile)
  const projectPath = useLayoutStore((s) => s.projectPath)

  const loadChildren = useCallback(async (dirPath: string): Promise<TreeNode[]> => {
    const entries = await listDirectory(dirPath)
    return entries.map((e) => ({
      entry: e,
      children: [],
      expanded: expandedPaths.has(e.path),
    }))
  }, [expandedPaths])

  const refreshTree = useCallback(async () => {
    try {
      const root = projectPath || '.'
      const entries = await listDirectory(root)
      const roots = entries.map((e) => ({
        entry: e,
        children: [] as TreeNode[],
        expanded: false,
      }))
      setRootNodes(roots)

      const status = await gitStatus()
      try {
        const parsed = JSON.parse(status)
        if (Array.isArray(parsed)) {
          const map: Record<string, string> = {}
          for (const item of parsed) {
            map[item.file] = item.status
          }
          setGitStatusMap(map)
        }
      } catch {}
    } catch {}
  }, [projectPath])

  useEffect(() => {
    refreshTree()
  }, [refreshTree])

  const handleToggle = async (path: string) => {
    const next = new Set(expandedPaths)
    if (next.has(path)) {
      next.delete(path)
    } else {
      next.add(path)
    }
    setExpandedPaths(next)

    const updateNode = async (nodes: TreeNode[]): Promise<TreeNode[]> => {
      return Promise.all(
        nodes.map(async (n) => {
          if (n.entry.path === path) {
            const children = next.has(path) ? await loadChildren(path) : []
            return { ...n, expanded: next.has(path), children }
          }
          if (n.entry.isDir && n.expanded) {
            return { ...n, children: await updateNode(n.children) }
          }
          return n
        })
      )
    }

    const updated = await updateNode(rootNodes)
    setRootNodes(updated)
  }

  const handleSelect = (entry: FileEntry) => {
    if (!entry.isDir) openFile(entry.path)
  }

  const handleRename = async (entry: FileEntry) => {
    const name = prompt('Rename:', entry.name)
    if (!name || name === entry.name) return
    const parent = entry.path.substring(0, entry.path.lastIndexOf('/'))
    const newPath = parent ? `${parent}/${name}` : name
    try {
      await renameItem(entry.path, newPath)
      await refreshTree()
    } catch (e) {
      console.error('Rename failed', e)
    }
  }

  const handleDelete = async (entry: FileEntry) => {
    if (!confirm(`Delete "${entry.name}"?`)) return
    try {
      await deleteItem(entry.path)
      await refreshTree()
    } catch (e) {
      console.error('Delete failed', e)
    }
  }

  const handleNewFile = async (parent: string) => {
    const name = prompt('File name:')
    if (!name) return
    const path = parent ? `${parent}/${name}` : name
    try {
      await writeFile(path, '')
      await refreshTree()
    } catch (e) {
      console.error('Create file failed', e)
    }
  }

  const handleNewFolder = async (parent: string) => {
    const name = prompt('Folder name:')
    if (!name) return
    const path = parent ? `${parent}/${name}` : name
    try {
      await createDir(path)
      await refreshTree()
    } catch (e) {
      console.error('Create folder failed', e)
    }
  }

  const applyGitStatus = (nodes: TreeNode[]): TreeNode[] =>
    nodes.map((n) => ({
      ...n,
      gitStatus: gitStatusMap[n.entry.path],
      children: applyGitStatus(n.children),
    }))

  const decorated = applyGitStatus(rootNodes)

  return (
    <div className="h-full overflow-auto py-1">
      {decorated.map((node) => (
        <TreeItem
          key={node.entry.path}
          node={node}
          depth={0}
          onToggle={handleToggle}
          onSelect={handleSelect}
          onRename={handleRename}
          onDelete={handleDelete}
          onNewFile={handleNewFile}
          onNewFolder={handleNewFolder}
        />
      ))}
    </div>
  )
}
