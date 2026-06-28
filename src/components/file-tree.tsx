import { useState, useEffect, useCallback } from 'react'
import {
  ChevronRight, ChevronDown,
  File, FileText, FileJson, FileCode, FileImage,
  FileType, FileTerminal, FileArchive, FileSpreadsheet,
  Folder, FolderOpen, FilePlus, FolderPlus,
  Pencil, Trash2, ExternalLink, Copy, Link, Eye,
} from 'lucide-react'
import { listDirectory, deleteItem, renameItem, createDir, gitStatus, writeFile, onFsChange } from '@/lib/ipc'
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

const extensionIcons: Record<string, React.ComponentType<{ className?: string }>> = {
  ts: FileCode, tsx: FileCode, js: FileCode, jsx: FileCode,
  mjs: FileCode, cjs: FileCode,
  rs: FileCode, go: FileCode, java: FileCode, kt: FileCode,
  swift: FileCode, py: FileCode, rb: FileCode, php: FileCode,
  c: FileCode, h: FileCode, cpp: FileCode, hpp: FileCode,
  cs: FileCode, fs: FileCode, dart: FileCode, lua: FileCode,
  scala: FileCode,
  json: FileJson, jsonc: FileJson,
  md: FileText, txt: FileText, log: FileText,
  css: FileCode, scss: FileCode, less: FileCode,
  html: FileCode, htm: FileCode, xml: FileCode,
  yaml: FileCode, yml: FileCode, toml: FileCode,
  svg: FileImage, png: FileImage, jpg: FileImage, jpeg: FileImage,
  gif: FileImage, bmp: FileImage, ico: FileImage, webp: FileImage, avif: FileImage,
  sh: FileTerminal, bash: FileTerminal, zsh: FileTerminal, fish: FileTerminal,
  sql: FileCode, graphql: FileCode, gql: FileCode,
  woff: FileType, woff2: FileType, ttf: FileType, otf: FileType, eot: FileType,
  wasm: FileCode,
  zip: FileArchive, tar: FileArchive, gz: FileArchive,
  bz2: FileArchive, xz: FileArchive, rar: FileArchive, '7z': FileArchive,
  pdf: FileText,
  csv: FileSpreadsheet, xlsx: FileSpreadsheet, xls: FileSpreadsheet,
}

function pickFileIcon(name: string): React.ComponentType<{ className?: string }> {
  const lower = name.toLowerCase()
  if (lower.startsWith('.') && lower.lastIndexOf('.') === 0) {
    return extensionIcons[lower.slice(1)] ?? File
  }
  const parts = lower.split('.')
  const ext = parts.length > 1 ? parts[parts.length - 1] : ''
  return extensionIcons[ext] ?? File
}

function getIcon(node: TreeNode) {
  if (node.entry.isDir) {
    return node.expanded
      ? <FolderOpen className="h-4 w-4 shrink-0 text-[#4A90D9]" />
      : <Folder className="h-4 w-4 shrink-0 text-[#4A90D9]" />
  }
  const Icon = pickFileIcon(node.entry.name)
  return <Icon className="h-4 w-4 shrink-0 text-[#607d8b]" />
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

function getParentDir(path: string): string {
  const normalized = path.replace(/\\/g, '/')
  const idx = normalized.lastIndexOf('/')
  return idx > 0 ? normalized.slice(0, idx) : ''
}

function isHtmlFile(name: string): boolean {
  const ext = name.split('.').pop()?.toLowerCase()
  return ext === 'html' || ext === 'htm'
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
  onPreview,
}: {
  node: TreeNode
  depth: number
  onToggle: (path: string) => void
  onSelect: (entry: FileEntry) => void
  onRename: (entry: FileEntry) => void
  onDelete: (entry: FileEntry) => void
  onNewFile: (parent: string) => void
  onNewFolder: (parent: string) => void
  onPreview: (entry: FileEntry) => void
}) {
  const hasChildren = node.entry.isDir
  const parentPath = hasChildren ? node.entry.path : getParentDir(node.entry.path)

  return (
    <>
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
            <span className="relative shrink-0">
              {getIcon(node)}
              {node.entry.isSymlink && (
                <Link className="h-2.5 w-2.5 absolute -top-1 -right-1.5 text-[#4A90D9]" />
              )}
            </span>
            <span className="truncate ml-1 flex-1 items-center gap-1 flex">
              {node.entry.name}
              {node.entry.isSymlink && (
                <span className="text-[10px] italic text-gray-400 shrink-0">symlink</span>
              )}
            </span>
            {getStatusDot(node.gitStatus)}
          </div>
        </ContextMenuTrigger>
        <ContextMenuContent className="w-48">
          <ContextMenuItem onClick={() => onNewFile(parentPath)}>
            <FilePlus className="h-4 w-4 mr-2" />
            New File
          </ContextMenuItem>
          <ContextMenuItem onClick={() => onNewFolder(parentPath)}>
            <FolderPlus className="h-4 w-4 mr-2" />
            New Folder
          </ContextMenuItem>
          <ContextMenuSeparator />
          <ContextMenuItem onClick={() => onRename(node.entry)}>
            <Pencil className="h-4 w-4 mr-2" />
            Rename
          </ContextMenuItem>
          <ContextMenuItem onClick={() => onDelete(node.entry)}>
            <Trash2 className="h-4 w-4 mr-2 text-red-500" />
            <span className="text-red-500">Delete</span>
          </ContextMenuItem>
          <ContextMenuItem onClick={() => navigator.clipboard.writeText(node.entry.path)}>
            <Copy className="h-4 w-4 mr-2" />
            Copy Path
          </ContextMenuItem>
          {!hasChildren && isHtmlFile(node.entry.name) && (
            <ContextMenuItem onClick={() => onPreview(node.entry)}>
              <Eye className="h-4 w-4 mr-2" />
              Preview
            </ContextMenuItem>
          )}
          <ContextMenuSeparator />
          <ContextMenuItem onClick={() => {/* reveal in explorer */}}>
            <ExternalLink className="h-4 w-4 mr-2" />
            Reveal in Explorer
          </ContextMenuItem>
        </ContextMenuContent>
      </ContextMenu>
      {node.expanded && node.children.map((child) => (
        <TreeItem
          key={child.entry.path}
          node={child}
          depth={depth + 1}
          onToggle={onToggle}
          onSelect={onSelect}
          onRename={onRename}
          onDelete={onDelete}
          onNewFile={onNewFile}
          onNewFolder={onNewFolder}
          onPreview={onPreview}
        />
      ))}
    </>
  )
}

export function FileTree() {
  const [rootNodes, setRootNodes] = useState<TreeNode[]>([])
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())
  const [gitStatusMap, setGitStatusMap] = useState<Record<string, string>>({})
  const [loadError, setLoadError] = useState<string | null>(null)
  const openFile = useEditorStore((s) => s.openFile)
  const setViewType = useEditorStore((s) => s.setViewType)
  const projectPath = useLayoutStore((s) => s.projectPath)

  const loadChildren = useCallback(async (dirPath: string, expanded: Set<string>): Promise<TreeNode[]> => {
    const entries = await listDirectory(dirPath)
    return entries.map((e) => ({
      entry: e,
      children: [],
      expanded: expanded.has(e.path),
    }))
  }, [])

  const refreshTree = useCallback(async () => {
    setLoadError(null)
    if (!projectPath) {
      setRootNodes([])
      return
    }
    try {
      const entries = await listDirectory(projectPath)
      const roots = entries.map((e) => ({
        entry: e,
        children: [] as TreeNode[],
        expanded: false,
      }))
      setRootNodes(roots)

      try {
        const entries = await gitStatus(projectPath)
        const map: Record<string, string> = {}
        for (const item of entries) {
          map[item.file] = item.status
        }
        setGitStatusMap(map)
      } catch {}
    } catch {
      setLoadError('Unable to list directory')
    }
  }, [projectPath])

  useEffect(() => {
    refreshTree()
  }, [refreshTree])

  useEffect(() => {
    let unlisten: (() => void) | undefined
    const setup = async () => {
      try {
        const result = await onFsChange(() => { refreshTree() })
        if (result && typeof result.unlisten === 'function') {
          unlisten = result.unlisten
        }
      } catch {}
    }
    setup()
    return () => {
      if (unlisten) try { unlisten() } catch {}
    }
  }, [refreshTree])

  const handleToggle = async (path: string) => {
    const next = new Set(expandedPaths)
    const isExpanding = !next.has(path)

    if (isExpanding) {
      next.add(path)
    } else {
      for (const p of next) {
        if (p === path || p.startsWith(path + '/') || p.startsWith(path + '\\')) {
          next.delete(p)
        }
      }
    }
    setExpandedPaths(next)

    const updateNode = async (nodes: TreeNode[]): Promise<TreeNode[]> => {
      return Promise.all(
        nodes.map(async (n) => {
          if (n.entry.path === path) {
            const children = next.has(path) ? await loadChildren(path, next) : []
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

  const handlePreview = (entry: FileEntry) => {
    openFile(entry.path)
    setViewType(entry.path, 'preview')
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
      {!projectPath && (
        <div className="px-3 py-4 text-sm text-gray-400 text-center">
          No project open
        </div>
      )}
      {loadError && (
        <div className="px-3 py-4 text-sm text-red-400 text-center">
          {loadError}
        </div>
      )}
      {projectPath && !loadError && decorated.length === 0 && (
        <div className="px-3 py-4 text-sm text-gray-400 text-center">
          Empty directory
        </div>
      )}
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
          onPreview={handlePreview}
        />
      ))}
    </div>
  )
}
