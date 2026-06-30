/**
 * Aggregates every story into the ordered list the gallery renders. Add new
 * component stories to their thematic file and they flow through here.
 */
import type { GalleryStory } from '../story'
import { compositeStories } from './composite.story'
import { controlStories } from './controls.story'
import { dataStories } from './data.story'
import { displayStories } from './display.story'
import { overlayStories } from './overlays.story'
import { typographyStories } from './typography.story'

export const ALL_STORIES: GalleryStory[] = [
  ...controlStories,
  ...displayStories,
  ...dataStories,
  ...overlayStories,
  ...typographyStories,
  ...compositeStories,
]
