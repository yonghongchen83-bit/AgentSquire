import { invoke } from '@tauri-apps/api/core'
import type { FileEntry, AppConfig } from '@/types/ipc'

export async function listDirectory(path: string): Promise<FileEntry[]> {
  return invoke('list_directory', { path })
}

export async function readFile(path: string): Promise<string> {
  return invoke('read_file', { path })
}

export async function writeFile(path: string, content: string): Promise<void> {
  return invoke('write_file', { path, content })
}

export async function createDir(path: string): Promise<void> {
  return invoke('create_dir', { path })
}

export async function deleteItem(path: string): Promise<void> {
  return invoke('delete_item', { path })
}

export async function renameItem(oldPath: string, newPath: string): Promise<void> {
  return invoke('rename_item', { oldPath, newPath })
}

export async function gitStatus(): Promise<string> {
  return invoke('git_status')
}

export async function loadConfig(): Promise<AppConfig> {
  return invoke('load_config')
}

export async function saveConfig(config: Partial<AppConfig>): Promise<void> {
  return invoke('save_config', { config })
}
