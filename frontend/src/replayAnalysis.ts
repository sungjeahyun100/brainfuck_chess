import type { TurnAction } from './types/game'
import type { AnalysisNode, AnalysisTree } from './types/gameRecord'

export interface AnalysisPositionRequest {
  base_ply: number
  tree_id?: string
  node_id?: string
  pending_actions?: TurnAction[]
}

/** Resolve the persisted cursor and the optimistic suffix independently. */
export function analysisPosition(
  tree: AnalysisTree | null,
  cursor: AnalysisNode | null,
  originalPly: number,
): AnalysisPositionRequest {
  if (!tree || !cursor) return { base_ply: originalPly }
  const pending: TurnAction[] = []
  let node: AnalysisNode | undefined = cursor
  while (node?.pending) {
    pending.unshift(node.action)
    node = node.parent_node_id
      ? tree.nodes.find(item => item.id === node!.parent_node_id)
      : undefined
  }
  return node && !tree.id.startsWith('local-')
    ? { base_ply: tree.base_ply, tree_id: tree.id, node_id: node.id, pending_actions: pending }
    : { base_ply: tree.base_ply, pending_actions: pending }
}

/** Replace exactly the acknowledged local node, preserving its descendants. */
export function reconcileOptimisticNode(
  tree: AnalysisTree,
  localId: string,
  persisted: AnalysisNode,
): AnalysisNode | null {
  const index = tree.nodes.findIndex(item => item.id === localId)
  if (index < 0) return null
  tree.nodes.forEach(item => {
    if (item.parent_node_id === localId) item.parent_node_id = persisted.id
  })
  const replacement = { ...persisted, pending: false }
  tree.nodes.splice(index, 1, replacement)
  return replacement
}
