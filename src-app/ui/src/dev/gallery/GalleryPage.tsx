/**
 * The component gallery — a dev-only canvas rendering every kit component across
 * its variants/states/tones/sizes plus a few composite scenes, under a
 * URL-driven theme × accent combo. The linchpin of the visual-testing system:
 * Layers A/B/C all run against this one stable surface.
 *
 * Dev-only: mounted at `/dev/gallery` (gated on `import.meta.env.DEV`) inside the
 * app shell, and served standalone (backend-free) at `/dev-gallery.html`.
 */
import { Flex, Select, Text, Title } from '@/components/ui'
import { ACCENT_PRESETS } from '@/components/ThemeProvider/accentPresets'
import { GALLERY_ALL_ACCENTS, GALLERY_DIRS, GALLERY_THEMES } from './matrix'
import { StorySection } from './story'
import { ALL_STORIES } from './stories'
import { GalleryPages } from './pages'
import { useGalleryTheme } from './useGalleryTheme'

const ctl = (name: string) => `gallery-control-${name}`

function ControlBar() {
  const { params, setTheme, setAccent, setDir } = useGalleryTheme()
  return (
    <div
      data-testid="gallery-controls"
      className="sticky top-0 z-10 flex flex-wrap items-end gap-4 border-b border-border bg-background/95 px-6 py-3 backdrop-blur"
    >
      <Title level={2} className="mr-auto">
        Component gallery
      </Title>
      <Flex direction="column" gap="xs">
        <Text tone="muted" className="text-xs uppercase tracking-wide">
          Theme
        </Text>
        <Select
          data-testid={ctl('theme')}
          aria-label="Theme"
          value={params.theme}
          onChange={v => setTheme(v as (typeof GALLERY_THEMES)[number])}
          options={GALLERY_THEMES.map(t => ({ value: t, label: t }))}
        />
      </Flex>
      <Flex direction="column" gap="xs">
        <Text tone="muted" className="text-xs uppercase tracking-wide">
          Accent
        </Text>
        <Select
          data-testid={ctl('accent')}
          aria-label="Accent"
          value={params.accent}
          onChange={v => setAccent(v as (typeof GALLERY_ALL_ACCENTS)[number])}
          options={GALLERY_ALL_ACCENTS.map(a => ({
            value: a,
            label: ACCENT_PRESETS[a].label,
          }))}
        />
      </Flex>
      <Flex direction="column" gap="xs">
        <Text tone="muted" className="text-xs uppercase tracking-wide">
          Direction
        </Text>
        <Select
          data-testid={ctl('dir')}
          aria-label="Direction"
          value={params.dir}
          onChange={v => setDir(v as (typeof GALLERY_DIRS)[number])}
          options={GALLERY_DIRS.map(d => ({ value: d, label: d.toUpperCase() }))}
        />
      </Flex>
    </div>
  )
}

export function GalleryPage() {
  return (
    <div
      data-testid="gallery-root"
      // overflow-x-hidden: the canvas chrome must not itself scroll horizontally
      // (a stress component overflowing its OWN box is still captured per-section
      // by Layer A's childOverflow + the Layer B section screenshot).
      className="min-h-full w-full overflow-x-hidden bg-background text-foreground"
    >
      <ControlBar />
      <div className="flex flex-col gap-6 p-6">
        {/* Seeded module pages — every route rendered populated via mock-API. */}
        <GalleryPages />
        {/* Isolated kit component stories. */}
        {ALL_STORIES.map(story => (
          <StorySection key={story.id} story={story} />
        ))}
      </div>
    </div>
  )
}

export default GalleryPage
