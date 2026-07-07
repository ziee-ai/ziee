/**
 * Aggregates every story into the ordered list the gallery renders. Add new
 * component stories to their thematic file and they flow through here.
 */
import type { GalleryStory } from '../story'
import { compositeStories } from './composite.story'
import { controlStories } from './controls.story'
import { dataStories } from './data.story'
import { displayStories } from './display.story'
import { mermaidStories } from './mermaid.story'
import { missingStories } from './missing.story'
import { overlayStories } from './overlays.story'
import { stressStories } from './stress.story'
import { typographyStories } from './typography.story'
// Per-shard story lanes (parallel gap grind) — each shard owns ONLY its file.
import { shard1Stories } from './shard1.story'
import { shard2Stories } from './shard2.story'
import { shard3Stories } from './shard3.story'
import { shard4Stories } from './shard4.story'
import { shard5Stories } from './shard5.story'

export const ALL_STORIES: GalleryStory[] = [
  ...controlStories,
  ...displayStories,
  ...dataStories,
  ...mermaidStories,
  ...overlayStories,
  ...typographyStories,
  ...compositeStories,
  ...stressStories,
  ...missingStories,
  ...shard1Stories,
  ...shard2Stories,
  ...shard3Stories,
  ...shard4Stories,
  ...shard5Stories,
]
