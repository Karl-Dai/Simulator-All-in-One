<script setup lang="ts">
import { ref, computed, nextTick, inject, watch, onMounted, onUnmounted, type Ref } from 'vue'
import * as App from '../../wailsjs/go/main/App'
import { useI18n, showAlert } from 'shared-frontend'
import NewConnectionDialog from './NewConnectionDialog.vue'
import NewScanGroupDialog from './NewScanGroupDialog.vue'
import type { MasterConnectionInfo, ScanGroupInfo } from '../types'

const { t } = useI18n()

const emit = defineEmits<{
  (e: 'connection-select', id: string, State: string): void
  (e: 'scan-group-select', connectionId: string, group: ScanGroupInfo): void
}>()

const treeRefreshKey = inject<Ref<number>>('treeRefreshKey')!
const refreshTree = inject<() => void>('refreshTree')!

interface TreeConnection {
  info: MasterConnectionInfo
  expanded: boolean
  scanGroups: ScanGroupInfo[]
}

const connections = ref<TreeConnection[]>([])
const selectedNodeId = ref<string | null>(null)
const editingConnectionId = ref<string | null>(null)
const editingGroup = ref<{ connId: string; group: ScanGroupInfo } | null>(null)
const menuElement = ref<HTMLElement | null>(null)
const contextConnection = computed(() => connections.value.find(c => c.info.ID === contextMenu.value.connId))
function dismissOutside(event: PointerEvent) {
  if (!menuElement.value?.contains(event.target as Node)) hideContextMenu()
}
function dismissEscape(event: KeyboardEvent) { if (event.key === 'Escape') hideContextMenu() }
onMounted(() => {
  document.addEventListener('pointerdown', dismissOutside)
  document.addEventListener('keydown', dismissEscape)
  window.addEventListener('resize', hideContextMenu)
})
onUnmounted(() => {
  document.removeEventListener('pointerdown', dismissOutside)
  document.removeEventListener('keydown', dismissEscape)
  window.removeEventListener('resize', hideContextMenu)
})

// Context menu
const contextMenu = ref<{ visible: boolean; x: number; y: number; type: 'connection' | 'scan_group'; connId: string; groupId?: string }>({
  visible: false, x: 0, y: 0, type: 'connection', connId: '', groupId: undefined
})

async function loadTree() {
  try {
    const conns = await App.ListConnections()
    const newTree: TreeConnection[] = []
    for (const conn of conns) {
      const existing = connections.value.find(c => c.info.ID === conn.ID)
      const groups = await App.ListScanGroups(conn.ID)
      newTree.push({
        info: conn,
        expanded: existing?.expanded ?? true,
        scanGroups: groups,
      })
    }
    connections.value = newTree
  } catch (_e) {
    // Ignore errors on load
  }
}

watch(treeRefreshKey, loadTree)
onMounted(loadTree)

function selectConnection(conn: TreeConnection) {
  selectedNodeId.value = conn.info.ID
  emit('connection-select', conn.info.ID, conn.info.State)
}

function selectScanGroup(conn: TreeConnection, group: ScanGroupInfo) {
  selectedNodeId.value = `${conn.info.ID}:${group.ID}`
  emit('connection-select', conn.info.ID, conn.info.State)
  emit('scan-group-select', conn.info.ID, group)
}

function toggleExpand(conn: TreeConnection) {
  conn.expanded = !conn.expanded
}

async function showContextMenu(e: MouseEvent, type: 'connection' | 'scan_group', connId: string, groupId?: string) {
  e.preventDefault()
  const conn = connections.value.find(c => c.info.ID === connId)
  if (!conn) return
  const group = conn.scanGroups.find(g => g.ID === groupId)
  if (group) selectScanGroup(conn, group)
  else selectConnection(conn)
  contextMenu.value = { visible: true, x: e.clientX, y: e.clientY, type, connId, groupId }
  await nextTick()
  const rect = menuElement.value?.getBoundingClientRect()
  if (rect) {
    contextMenu.value.x = Math.max(4, Math.min(e.clientX, window.innerWidth - rect.width - 4))
    contextMenu.value.y = Math.max(4, Math.min(e.clientY, window.innerHeight - rect.height - 4))
  }
  menuElement.value?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus()
}

function hideContextMenu() {
  contextMenu.value.visible = false
}

function ctxEditConnection() {
  if (contextConnection.value?.info.State !== 'Disconnected') return
  editingConnectionId.value = contextMenu.value.connId
  hideContextMenu()
}
function ctxEditScanGroup() {
  const group = contextConnection.value?.scanGroups.find(g => g.ID === contextMenu.value.groupId)
  if (!group) return
  editingGroup.value = { connId: contextMenu.value.connId, group: { ...group } }
  hideContextMenu()
}
async function connectionSaved(id: string) {
  await loadTree()
  const conn = connections.value.find(c => c.info.ID === id)
  if (conn) selectConnection(conn)
  refreshTree()
}
function groupSaved(group: ScanGroupInfo) {
  const conn = connections.value.find(c => c.info.ID === editingGroup.value?.connId)
  if (conn) selectScanGroup(conn, group)
  refreshTree()
}
async function ctxDisconnect() {
  const id = contextMenu.value.connId
  hideContextMenu()
  try { await App.Disconnect(id); refreshTree() }
  catch (e) { await showAlert(String(e)) }
}

async function ctxStartPolling() {
  if (!contextMenu.value.groupId) return
  try {
    await App.StartPolling(contextMenu.value.connId, contextMenu.value.groupId)
    refreshTree()
  } catch (_e) { /* ignore */ }
  hideContextMenu()
}

async function ctxStopPolling() {
  if (!contextMenu.value.groupId) return
  try {
    await App.StopPolling(contextMenu.value.connId, contextMenu.value.groupId)
    refreshTree()
  } catch (_e) { /* ignore */ }
  hideContextMenu()
}

async function ctxRemoveScanGroup() {
  if (!contextMenu.value.groupId) return
  try {
    await App.RemoveScanGroup(contextMenu.value.connId, contextMenu.value.groupId)
    refreshTree()
  } catch (_e) { /* ignore */ }
  hideContextMenu()
}

async function ctxDeleteConnection() {
  try {
    await App.DeleteConnection(contextMenu.value.connId)
    refreshTree()
  } catch (_e) { /* ignore */ }
  hideContextMenu()
}

function fcLabel(fn: string): string {
  const map: Record<string, string> = {
    read_coils: 'FC01',
    read_discrete_inputs: 'FC02',
    read_holding_registers: 'FC03',
    read_input_registers: 'FC04',
  }
  return map[fn] || fn
}
</script>

<template>
  <div class="tree-container" @click="hideContextMenu">
    <div class="tree-header">{{ t('tree.connections') }}</div>

    <div v-if="connections.length === 0" class="tree-empty">
      {{ t('tree.noConnection') }}
    </div>

    <div v-for="conn in connections" :key="conn.info.ID" class="tree-node-group">
      <!-- Connection node -->
      <div
        :class="['tree-node', { selected: selectedNodeId === conn.info.ID }]"
        role="button" tabindex="0" @click="selectConnection(conn)" @keydown.enter.prevent="selectConnection(conn)" @keydown.space.prevent="selectConnection(conn)"
        @contextmenu="showContextMenu($event, 'connection', conn.info.ID)"
      >
        <span class="node-expand" @click.stop="toggleExpand(conn)">
          {{ conn.expanded ? '▼' : '▶' }}
        </span>
        <span :class="['node-status', conn.info.State.toLowerCase()]"></span>
        <span class="node-label">{{ conn.info.TargetAddress }}:{{ conn.info.Port }}</span>
        <span class="node-slave">ID:{{ conn.info.SlaveID }}</span>
      </div>

      <!-- Scan group children -->
      <div v-if="conn.expanded" class="tree-children">
        <div
          v-for="group in conn.scanGroups"
          :key="group.ID"
          :class="['tree-node', 'tree-child', { selected: selectedNodeId === `${conn.info.ID}:${group.ID}` }]"
          role="button" tabindex="0" @click="selectScanGroup(conn, group)" @keydown.enter.prevent="selectScanGroup(conn, group)" @keydown.space.prevent="selectScanGroup(conn, group)"
          @contextmenu="showContextMenu($event, 'scan_group', conn.info.ID, group.ID)"
        >
          <span :class="['poll-indicator', { active: group.IsPolling }]"></span>
          <span class="node-label">{{ group.Name }}</span>
          <span class="node-meta">{{ t('dialog.slaveId') }}:{{ group.SlaveID ?? conn.info.SlaveID }} · {{ fcLabel(group.Function) }} {{ group.StartAddress }}x{{ group.Quantity }}</span>
        </div>
      </div>
    </div>

    <!-- Context Menu -->
    <div v-if="contextMenu.visible" ref="menuElement" role="menu" class="context-menu" :style="{ left: contextMenu.x + 'px', top: contextMenu.y + 'px' }">
      <template v-if="contextMenu.type === 'connection'">
        <button role="menuitem" class="ctx-item" :disabled="contextConnection?.info.State !== 'Disconnected'" :title="t('dialog.editConnectionHint')" @click="ctxEditConnection">{{ t('parity.connectionSettings') }}</button>
        <button v-if="contextConnection?.info.State !== 'Disconnected'" role="menuitem" class="ctx-item" @click="ctxDisconnect">{{ t('toolbar.disconnect') }}</button>
        <button role="menuitem" class="ctx-item danger" @click="ctxDeleteConnection">{{ t('tree.deleteConnection') }}</button>
      </template>
      <template v-else>
        <button role="menuitem" class="ctx-item" @click="ctxEditScanGroup">{{ t('dialog.editScanGroup') }}</button>
        <button role="menuitem" class="ctx-item" @click="ctxStartPolling">{{ t('tree.startPolling') }}</button>
        <button role="menuitem" class="ctx-item" @click="ctxStopPolling">{{ t('tree.stopPolling') }}</button>
        <button role="menuitem" class="ctx-item danger" @click="ctxRemoveScanGroup">{{ t('tree.deleteScanGroup') }}</button>
      </template>
    </div>
  </div>
  <NewConnectionDialog v-if="editingConnectionId" :show="true" :connection-id="editingConnectionId"
    @close="editingConnectionId = null" @saved="connectionSaved(contextMenu.connId)" />
  <NewScanGroupDialog v-if="editingGroup" :show="true" :connection-id="editingGroup.connId" :group="editingGroup.group"
    @close="editingGroup = null" @saved="groupSaved" />
</template>

<style scoped>
.tree-node:focus-visible { outline: 2px solid var(--c-blue); outline-offset: -2px; }
.tree-container {
  padding: 4px 0;
  font-size: 12px;
  user-select: none;
}

.tree-header {
  padding: 8px 12px;
  font-size: 11px;
  text-transform: uppercase;
  color: var(--c-subtext0);
  letter-spacing: 0.5px;
}

.tree-empty {
  padding: 24px 12px;
  color: var(--c-subtext0);
  text-align: center;
}

.tree-node {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 8px;
  cursor: pointer;
  border-radius: 3px;
  margin: 1px 4px;
}

.tree-node:hover {
  background: var(--c-hover-bg);
}

.tree-node.selected {
  background: var(--c-selection-bg);
  color: var(--c-selection-text);
}

.tree-node.selected .node-meta,
.tree-node.selected .node-slave {
  color: var(--c-selection-text);
  opacity: 1;
}

.tree-child {
  padding-left: 28px;
}

.node-expand {
  font-size: 8px;
  width: 12px;
  text-align: center;
  color: var(--c-subtext0);
}

.node-status {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.node-status.connected { background: var(--c-green); }
.node-status.disconnected { background: var(--c-red); }
.node-status.Error { background: var(--c-peach); }
.node-status.reconnecting {
  background: var(--c-yellow);
  animation: reconnecting-pulse 1s ease-in-out infinite;
}

@keyframes reconnecting-pulse {
  0%, 100% { opacity: 0.4; }
  50% { opacity: 1; }
}

.poll-indicator {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
  background: var(--c-surface1);
}

.poll-indicator.active {
  background: var(--c-green);
  box-shadow: 0 0 4px var(--c-green);
}

.node-label {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.node-slave {
  font-size: 10px;
  color: var(--c-subtext0);
}

.node-meta {
  font-size: 10px;
  color: var(--c-subtext0);
  font-family: monospace;
}

/* Context Menu */
.context-menu {
  position: fixed;
  background: var(--c-base);
  border: 1px solid var(--c-surface1);
  border-radius: 6px;
  padding: 4px 0;
  z-index: 999;
  min-width: 120px;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
}

.ctx-item {
  display: block; width: 100%; text-align: left; border: 0; background: transparent; color: var(--c-text); font-family: inherit;
  padding: 6px 14px;
  cursor: pointer;
  font-size: 12px;
}

.ctx-item:disabled { opacity: 0.45; cursor: not-allowed; }
.ctx-item:focus-visible { outline: 2px solid var(--c-blue); outline-offset: -2px; }
.ctx-item:hover:not(:disabled) {
  background: var(--c-control-bg);
}

.ctx-item.danger {
  color: var(--c-red);
}
</style>
