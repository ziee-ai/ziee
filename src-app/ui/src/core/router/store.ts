import { create } from 'zustand'
import type {
  RouteConfig,
  AppModule,
  SidebarActionButton,
  SidebarNavItem,
  SidebarWidget,
  SettingsMenuItem,
  GlobalComponent,
  WidgetSlots,
} from './types'
import { createStoreProxy } from '../stores'
import './types-store' // Register Router store type

interface RouterState {
  routes: RouteConfig[]
  modules: AppModule[]
  stores: Record<string, any>
  sidebarItems: {
    primaryActions: SidebarActionButton[]
    navigation: SidebarNavItem[]
    tools: SidebarNavItem[]
    widgets: Map<string, SidebarWidget[]>
  }
  settingsItems: SettingsMenuItem[]
  globalComponents: GlobalComponent[]
  widgets: Map<keyof WidgetSlots, any[]> // General widget registry by slot (e.g., 'userGroup', 'dashboard')
  registerModule: (module: AppModule) => void
  initializeModules: () => void
}

export const useRouterStore = create<RouterState>((set, get) => ({
  routes: [],
  modules: [],
  stores: {},
  sidebarItems: {
    primaryActions: [],
    navigation: [],
    tools: [],
    widgets: new Map(),
  },
  settingsItems: [],
  globalComponents: [],
  widgets: new Map(),

  registerModule: (module: AppModule) => {
    set(state => {
      // Check if module is already registered
      const existingIndex = state.modules.findIndex(
        m => m.metadata.name === module.metadata.name,
      )

      if (existingIndex !== -1) {
        // In development, allow re-registration for HMR
        if (import.meta.env.DEV) {
          console.log(
            `🔄 Re-registering module for HMR: ${module.metadata.name}`,
          )

          // Remove old module and its routes
          const oldModule = state.modules[existingIndex]
          const oldRoutes = oldModule.registerRoutes()
          const oldRoutePaths = new Set(oldRoutes.map(r => r.path))

          const newModules = [...state.modules]
          newModules[existingIndex] = module

          // Remove old routes and add new ones
          const filteredRoutes = state.routes.filter(
            r => !oldRoutePaths.has(r.path),
          )
          const moduleRoutes = module.registerRoutes()
          const newRoutes = [...filteredRoutes, ...moduleRoutes]

          // Re-register stores
          const newStores = { ...state.stores }
          if (module.registerStores) {
            const storeRegistrations = module.registerStores()
            storeRegistrations.forEach(reg => {
              newStores[reg.name] = createStoreProxy(reg.store)
            })
          }

          // Re-register sidebar items - remove old ones first
          // Get old sidebar items from the old module
          const oldSidebar = oldModule.registerSidebar?.()
          const oldActionIds = new Set(
            oldSidebar?.primaryActions?.map(a => a.id) || [],
          )
          const oldNavIds = new Set(
            oldSidebar?.navigation?.map(n => n.id) || [],
          )
          const oldToolIds = new Set(oldSidebar?.tools?.map(t => t.id) || [])

          // Filter out old items
          const newSidebarItems = {
            primaryActions: state.sidebarItems.primaryActions.filter(
              a => !oldActionIds.has(a.id),
            ),
            navigation: state.sidebarItems.navigation.filter(
              n => !oldNavIds.has(n.id),
            ),
            tools: state.sidebarItems.tools.filter(t => !oldToolIds.has(t.id)),
            widgets: new Map(state.sidebarItems.widgets),
          }

          // Remove old widgets
          if (oldSidebar?.widgets) {
            oldSidebar.widgets.forEach(oldWidget => {
              const slotWidgets =
                newSidebarItems.widgets.get(oldWidget.slot) || []
              newSidebarItems.widgets.set(
                oldWidget.slot,
                slotWidgets.filter(w => w.id !== oldWidget.id),
              )
            })
          }

          // Add new sidebar items
          if (module.registerSidebar) {
            const sidebar = module.registerSidebar()
            if (sidebar.primaryActions) {
              newSidebarItems.primaryActions.push(...sidebar.primaryActions)
            }
            if (sidebar.navigation) {
              newSidebarItems.navigation.push(...sidebar.navigation)
            }
            if (sidebar.tools) {
              newSidebarItems.tools.push(...sidebar.tools)
            }
            if (sidebar.widgets) {
              sidebar.widgets.forEach(widget => {
                const existing = newSidebarItems.widgets.get(widget.slot) || []
                newSidebarItems.widgets.set(widget.slot, [...existing, widget])
              })
            }
          }

          // Re-register settings items - remove old ones first
          const oldSettings = oldModule.registerSettings?.()
          const oldSettingsIds = new Set(oldSettings?.map(s => s.id) || [])

          // Filter out old items
          let newSettingsItems = state.settingsItems.filter(
            s => !oldSettingsIds.has(s.id),
          )

          // Add new settings items
          if (module.registerSettings) {
            const settings = module.registerSettings()
            newSettingsItems = [...newSettingsItems, ...settings]
          }

          // Re-register global components - remove old ones first
          const oldGlobalComponents = oldModule.registerGlobalComponents?.()
          const oldGlobalComponentIds = new Set(
            oldGlobalComponents?.map(c => c.id) || [],
          )

          // Filter out old items
          let newGlobalComponents = state.globalComponents.filter(
            c => !oldGlobalComponentIds.has(c.id),
          )

          // Add new global components
          if (module.registerGlobalComponents) {
            const globalComponents = module.registerGlobalComponents()
            newGlobalComponents = [...newGlobalComponents, ...globalComponents]
            // Sort by order
            newGlobalComponents.sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
          }

          // Re-register widgets - rebuild from all modules
          // Since widgets don't have IDs, we can't selectively remove them
          // Instead, rebuild the entire widget registry from all modules
          const newWidgets = new Map<keyof WidgetSlots, any[]>()

          // Register widgets from all modules (including the updated one)
          for (const mod of newModules) {
            if (mod.registerWidgets) {
              const widgets = mod.registerWidgets()
              for (const [slotName, widgetArray] of Object.entries(widgets)) {
                const slot = slotName as keyof WidgetSlots
                const existing = newWidgets.get(slot) || []
                newWidgets.set(slot, [...existing, ...widgetArray])
              }
            }
          }

          return {
            modules: newModules,
            routes: newRoutes,
            stores: newStores,
            sidebarItems: newSidebarItems,
            settingsItems: newSettingsItems,
            globalComponents: newGlobalComponents,
            widgets: newWidgets,
          }
        } else {
          console.warn(`Module ${module.metadata.name} is already registered`)
          return state
        }
      }

      // Register the module
      const newModules = [...state.modules, module]

      // Get routes from the module
      const moduleRoutes = module.registerRoutes()
      const newRoutes = [...state.routes, ...moduleRoutes]

      // Get stores from the module
      const newStores = { ...state.stores }
      if (module.registerStores) {
        const storeRegistrations = module.registerStores()
        storeRegistrations.forEach(reg => {
          newStores[reg.name] = createStoreProxy(reg.store)
        })
      }

      // Get sidebar items from the module
      const newSidebarItems = {
        primaryActions: [...state.sidebarItems.primaryActions],
        navigation: [...state.sidebarItems.navigation],
        tools: [...state.sidebarItems.tools],
        widgets: new Map(state.sidebarItems.widgets),
      }

      if (module.registerSidebar) {
        const sidebar = module.registerSidebar()
        if (sidebar.primaryActions) {
          newSidebarItems.primaryActions.push(...sidebar.primaryActions)
        }
        if (sidebar.navigation) {
          newSidebarItems.navigation.push(...sidebar.navigation)
        }
        if (sidebar.tools) {
          newSidebarItems.tools.push(...sidebar.tools)
        }
        if (sidebar.widgets) {
          sidebar.widgets.forEach(widget => {
            const existing = newSidebarItems.widgets.get(widget.slot) || []
            newSidebarItems.widgets.set(widget.slot, [...existing, widget])
          })
        }
      }

      // Get settings items from the module
      const newSettingsItems = [...state.settingsItems]
      if (module.registerSettings) {
        const settings = module.registerSettings()
        newSettingsItems.push(...settings)
      }

      // Get global components from the module
      const newGlobalComponents = [...state.globalComponents]
      if (module.registerGlobalComponents) {
        const globalComponents = module.registerGlobalComponents()
        newGlobalComponents.push(...globalComponents)
        // Sort by order
        newGlobalComponents.sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
      }

      console.log(`Registered module: ${module.metadata.name}`, {
        routes: moduleRoutes.length,
        stores: module.registerStores ? module.registerStores().length : 0,
        sidebar: module.registerSidebar ? 'yes' : 'no',
        settings: module.registerSettings
          ? module.registerSettings().length
          : 0,
        globalComponents: module.registerGlobalComponents
          ? module.registerGlobalComponents().length
          : 0,
      })

      return {
        modules: newModules,
        routes: newRoutes,
        stores: newStores,
        sidebarItems: newSidebarItems,
        settingsItems: newSettingsItems,
        globalComponents: newGlobalComponents,
      }
    })
  },

  initializeModules: () => {
    const { modules } = get()

    // Step 0: Register the Router store itself in the stores registry
    set(state => ({
      stores: {
        ...state.stores,
        Router: createStoreProxy(useRouterStore),
      },
    }))

    // Step 1: Run module initialize functions first (creates widget slots/registries)
    for (const module of modules) {
      if (module.initialize) {
        try {
          const result = module.initialize()
          // If initialize returns a promise, handle it but don't await
          if (result instanceof Promise) {
            result
              .then(() =>
                console.log(`Initialized module: ${module.metadata.name}`),
              )
              .catch(error =>
                console.error(
                  `Failed to initialize module ${module.metadata.name}:`,
                  error,
                ),
              )
          } else {
            console.log(`Initialized module: ${module.metadata.name}`)
          }
        } catch (error) {
          console.error(
            `Failed to initialize module ${module.metadata.name}:`,
            error,
          )
        }
      }
    }

    // Step 2: Register widgets from all modules
    // Rebuild from scratch to prevent duplication during HMR
    set(state => {
      const widgetsMap = new Map<keyof WidgetSlots, any[]>()

      for (const module of modules) {
        if (module.registerWidgets) {
          try {
            const widgets = module.registerWidgets()

            // Register widgets for each slot
            for (const [slotName, widgetArray] of Object.entries(widgets)) {
              const slot = slotName as keyof WidgetSlots
              const existing = widgetsMap.get(slot) || []
              widgetsMap.set(slot, [...existing, ...widgetArray])
              console.log(`✅ Registered ${widgetArray.length} widget(s) for slot: ${slotName}`)
            }
          } catch (error) {
            console.error(
              `Failed to register widgets for module ${module.metadata.name}:`,
              error,
            )
          }
        }
      }

      return { widgets: widgetsMap }
    })
  },
}))
