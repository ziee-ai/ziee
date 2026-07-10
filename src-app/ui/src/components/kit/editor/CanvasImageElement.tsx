import { PlateElement, type PlateElementProps } from 'platejs/react'

/**
 * Minimal render for a Plate `img` void node in the markdown canvas — a plain
 * `<img>` (no plate-ui resize chrome) pointing at the uploaded file's raw-bytes
 * route. The `@platejs/markdown` serializer turns this node back into a markdown
 * `![](url)` on Save, so the embed survives round-trip + reload (ITEM-21).
 */
export function CanvasImageElement(props: PlateElementProps) {
  const url = (props.element as { url?: string }).url ?? ''
  return (
    <PlateElement {...props}>
      <img
        src={url}
        alt=""
        data-testid="canvas-image"
        contentEditable={false}
        className="my-2 max-w-full rounded border border-border"
      />
      {props.children}
    </PlateElement>
  )
}
