/**
 * Re-export of the `@ziee/gallery` story contract + layout primitives, kept at
 * this path so the ziee kit-component stories (`stories/*.story.tsx`) import
 * `../story` unchanged after the framework extraction.
 */
export type { GalleryStory, GalleryCase } from '@ziee/gallery'
export { StorySection, sectionTestId, caseTestId } from '@ziee/gallery'
