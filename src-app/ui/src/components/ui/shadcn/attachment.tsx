import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"
import { Slot } from "@radix-ui/react-slot"

import { cn } from "@/lib/utils"
import { Button } from "./button"

const attachmentVariants = cva(
  "cn-attachment group/attachment relative flex w-fit max-w-full min-w-0 shrink-0 flex-wrap rounded-xl border bg-card text-card-foreground transition-colors has-[>a,>button]:hover:bg-muted/50 has-[>[data-slot=attachment-trigger]]:hover:bg-muted/50 data-[state=error]:border-destructive/30 data-[state=idle]:border-dashed",
  {
    variants: {
      size: {
        default: "cn-attachment-size-default gap-2 text-sm has-data-[slot=attachment-content]:px-2.5 has-data-[slot=attachment-content]:py-2 has-data-[slot=attachment-media]:p-2",
        sm: "cn-attachment-size-sm gap-2.5 text-xs has-data-[slot=attachment-content]:px-2 has-data-[slot=attachment-content]:py-1.5 has-data-[slot=attachment-media]:p-1.5",
        xs: "cn-attachment-size-xs gap-1.5 rounded-lg text-xs has-data-[slot=attachment-content]:px-1.5 has-data-[slot=attachment-content]:py-1 has-data-[slot=attachment-media]:p-1",
      },
      orientation: {
        horizontal: "cn-attachment-orientation-horizontal items-center min-w-40",
        vertical: "cn-attachment-orientation-vertical flex-col w-24 has-data-[slot=attachment-content]:w-30",
        squareImage:
          "cn-attachment-orientation-square-image relative aspect-square w-24 flex-col flex-nowrap gap-0 overflow-hidden p-0 has-data-[slot=attachment-content]:p-0",
      },
    },
  }
)

function Attachment({
  className,
  state = "done",
  size = "default",
  orientation = "horizontal",
  ...props
}: React.ComponentProps<"div"> &
  VariantProps<typeof attachmentVariants> & {
    state?: "idle" | "uploading" | "processing" | "error" | "done"
  }) {
  const resolvedOrientation = orientation ?? "horizontal"

  return (
    <div
      data-slot="attachment"
      data-state={state}
      data-size={size}
      data-orientation={resolvedOrientation}
      className={cn(attachmentVariants({ size, orientation }), className)}
      {...props}
    />
  )
}

const attachmentMediaVariants = cva(
  "cn-attachment-media relative flex aspect-square w-10 shrink-0 items-center justify-center overflow-hidden rounded-lg bg-muted text-foreground [&_svg:not([class*='size-'])]:size-4 group-data-[size=sm]/attachment:w-8 group-data-[size=xs]/attachment:w-7 group-data-[orientation=vertical]/attachment:w-full group-data-[orientation=squareImage]/attachment:absolute group-data-[orientation=squareImage]/attachment:inset-0 group-data-[orientation=squareImage]/attachment:w-full group-data-[orientation=squareImage]/attachment:rounded-none group-data-[orientation=squareImage]/attachment:[&_svg:not([class*='size-'])]:size-7 group-data-[state=error]/attachment:bg-destructive/10 group-data-[state=error]/attachment:text-destructive [&_svg]:pointer-events-none",
  {
    variants: {
      variant: {
        icon: "cn-attachment-media-variant-icon",
        image:
          "cn-attachment-media-variant-image *:[img]:aspect-square *:[img]:w-full *:[img]:object-cover",
      },
    },
    defaultVariants: {
      variant: "icon",
    },
  }
)

function AttachmentMedia({
  className,
  variant = "icon",
  ...props
}: React.ComponentProps<"div"> & VariantProps<typeof attachmentMediaVariants>) {
  return (
    <div
      data-slot="attachment-media"
      data-variant={variant}
      className={cn(attachmentMediaVariants({ variant }), className)}
      {...props}
    />
  )
}

function AttachmentContent({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="attachment-content"
      className={cn(
        "cn-attachment-content max-w-full min-w-0 flex-1 leading-tight",
        className
      )}
      {...props}
    />
  )
}

function AttachmentTitle({
  className,
  ...props
}: React.ComponentProps<"span">) {
  return (
    <span
      data-slot="attachment-title"
      className={cn(
        "cn-attachment-title block max-w-full min-w-0 truncate font-medium",
        className
      )}
      {...props}
    />
  )
}

function AttachmentDescription({
  className,
  ...props
}: React.ComponentProps<"span">) {
  return (
    <span
      data-slot="attachment-description"
      className={cn(
        "cn-attachment-description mt-0.5 block min-w-0 truncate text-xs text-muted-foreground group-data-[state=error]/attachment:text-destructive/80",
        "max-w-full",
        className
      )}
      {...props}
    />
  )
}

function AttachmentActions({
  className,
  ...props
}: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="attachment-actions"
      className={cn(
        "cn-attachment-actions relative z-20 flex shrink-0 items-center group-data-[orientation=vertical]/attachment:absolute group-data-[orientation=vertical]/attachment:top-2 group-data-[orientation=vertical]/attachment:right-2 group-data-[orientation=vertical]/attachment:gap-1 group-data-[orientation=squareImage]/attachment:absolute group-data-[orientation=squareImage]/attachment:top-2 group-data-[orientation=squareImage]/attachment:right-2 group-data-[orientation=squareImage]/attachment:gap-1 group-data-[orientation=squareImage]/attachment:z-20",
        className
      )}
      {...props}
    />
  )
}

function AttachmentAction({
  className,
  variant,
  size = "icon",
  ...props
}: React.ComponentProps<typeof Button>) {
  return (
    <Button
      data-slot="attachment-action"
      variant={variant ?? "ghost"}
      size={size}
      className={cn("cn-attachment-action", className)}
      {...props}
    />
  )
}

function AttachmentTrigger({
  className,
  asChild = false,
  type,
  ...props
}: React.ComponentProps<"button"> & {
  asChild?: boolean
}) {
  const Comp = asChild ? Slot : "button"

  return (
    <Comp
      data-slot="attachment-trigger"
      type={asChild ? undefined : (type ?? "button")}
      className={cn(
        "cn-attachment-trigger absolute inset-0 z-10 rounded-[inherit] outline-none focus-visible:ring-2 focus-visible:ring-ring",
        className
      )}
      {...props}
    />
  )
}

function AttachmentGroup({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="attachment-group"
      className={cn(
        "cn-attachment-group flex min-w-0 gap-3 py-1 snap-x snap-mandatory overflow-x-auto overscroll-x-contain *:data-[slot=attachment]:flex-none *:data-[slot=attachment]:snap-start",
        className
      )}
      {...props}
    />
  )
}

export {
  Attachment,
  AttachmentGroup,
  AttachmentMedia,
  AttachmentContent,
  AttachmentTitle,
  AttachmentDescription,
  AttachmentActions,
  AttachmentAction,
  AttachmentTrigger,
}
