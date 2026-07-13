import type { ModuleGallery } from '@/dev/gallery/support'

// The first-run SetupPage reads `App.getSetupStatus` on mount; seed a
// needs-setup response so the setup surface renders in the gallery.
export const gallery: ModuleGallery = {
  cassette: {
    'App.getSetupStatus': { needs_setup: true },
  },
}
