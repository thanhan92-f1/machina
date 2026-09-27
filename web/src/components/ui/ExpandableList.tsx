// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useId, useState, type ReactNode } from 'react'
import { DEFAULT_TOP_N, visibleItems } from '../../utils/topN'

type Props<T> = {
  items: readonly T[]
  renderItem: (item: T, index: number) => ReactNode
  /** Rows shown before the toggle. */
  limit?: number
  /** Optional ranking applied before truncating (e.g. `bySeverityDesc`). */
  compare?: (a: T, b: T) => number
  /** Plural noun for the toggle label, e.g. "events". */
  noun?: string
  className?: string
}

/**
 * A long list that shows its top rows and expands on request. The toggle reports its state
 * (`aria-expanded`) and controls the list it reveals. Lists at or under `limit` render as-is,
 * with no toggle.
 */
export function ExpandableList<T>({ items, renderItem, limit = DEFAULT_TOP_N, compare, noun = 'items', className }: Props<T>) {
  const [expanded, setExpanded] = useState(false)
  const listId = useId()
  const { shown, hidden } = visibleItems(items, expanded, limit, compare)
  return (
    <>
      <div id={listId} className={className}>
        {shown.map((item, i) => renderItem(item, i))}
      </div>
      {items.length > limit && (
        <button
          type="button"
          className="btn-secondary text-sm mt-3"
          aria-expanded={expanded}
          aria-controls={listId}
          onClick={() => setExpanded((v) => !v)}
        >
          {expanded ? `Show fewer ${noun}` : `Show ${hidden} more ${noun}`}
        </button>
      )}
    </>
  )
}
