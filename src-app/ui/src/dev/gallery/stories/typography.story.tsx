/**
 * Stories for typography primitives.
 */
import { Link, Paragraph, Text, Title } from '@ziee/kit'
import type { GalleryStory } from '../story'

const titleStory: GalleryStory = {
  id: 'title',
  title: 'Title',
  cases: [
    {
      key: 'levels',
      label: 'Levels 1–5',
      render: () => (
        <div className="flex flex-col gap-1">
          {([1, 2, 3, 4, 5] as const).map(l => (
            <Title key={l} level={l}>
              Title level {l}
            </Title>
          ))}
        </div>
      ),
    },
  ],
}

const textStory: GalleryStory = {
  id: 'text',
  title: 'Text',
  cases: [
    {
      key: 'tones',
      label: 'Tones / strong / code',
      render: () => (
        <div className="flex flex-col gap-1">
          <Text>Default text</Text>
          <Text tone="secondary">Secondary text</Text>
          <Text tone="muted">Muted text</Text>
          <Text tone="success">Success text</Text>
          <Text tone="warning">Warning text</Text>
          <Text tone="danger">Danger text</Text>
          <Text strong>Strong text</Text>
          <Text code>const code = true</Text>
        </div>
      ),
    },
  ],
}

const paragraphStory: GalleryStory = {
  id: 'paragraph',
  title: 'Paragraph',
  cases: [
    {
      key: 'basic',
      label: 'Basic',
      render: () => (
        <Paragraph className="max-w-md">
          A paragraph of body copy that runs across a couple of lines so we can
          check leading, measure, and wrapping behavior under each theme.
        </Paragraph>
      ),
    },
  ],
}

const linkStory: GalleryStory = {
  id: 'link',
  title: 'Link',
  cases: [
    {
      key: 'basic',
      label: 'Inline',
      render: () => (
        <Paragraph>
          Visit the{' '}
          <Link href="https://example.com" target="_blank">
            documentation
          </Link>{' '}
          for more.
        </Paragraph>
      ),
    },
  ],
}

export const typographyStories: GalleryStory[] = [
  titleStory,
  textStory,
  paragraphStory,
  linkStory,
]
