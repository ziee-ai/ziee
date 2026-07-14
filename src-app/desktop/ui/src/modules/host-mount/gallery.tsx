/**
 * Desktop dev-gallery seed for the `host-mount` module — seeds the host-mount
 * policy the settings page loads so it renders populated.
 */
import type { ModuleGallery } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  cassette: {
    'HostMount.getPolicy': {
      enabled: true,
      allow_readwrite: true,
      allowed_prefixes: ['/home/user/projects', '/data/shared'],
    },
  },
}
