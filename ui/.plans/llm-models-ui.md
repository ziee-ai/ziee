# LLM Models UI Implementation Plan

## Overview

Add LLM Model management UI to the existing `llm-provider` module following ziee-chat architecture patterns.

**Key Principles:**
- ✅ Use `ApiClient` from `@/api-client` (NO axios)
- ✅ Follow ziee-chat module patterns (`createModule`, `Stores`, etc.)
- ✅ Integrate into existing `llm-provider` module
- ✅ Use Zustand with `subscribeWithSelector` for state
- ✅ Use auto-generated types from OpenAPI spec

---

## File Mapping: react-test → ziee-chat

This section maps every react-test file to its corresponding ziee-chat location, showing the exact source for each component.

### **Store Files**

| react-test Source | ziee-chat Destination | Notes |
|-------------------|----------------------|-------|
| `/src/store/admin/providers.ts` (549 lines) | `ui/src/modules/llm-provider/store.ts` | Already exists - LLM model operations to be added |
| `/src/store/admin/modelDownload.ts` (346 lines) | `ui/src/modules/llm-provider/llm-model-download-store.ts` | NEW - Download tracking with SSE |
| `/src/store/ui/addLocalModelUploadDrawer.ts` (36 lines) | `ui/src/modules/llm-provider/llm-model-drawer-store.ts` | NEW - Part of combined drawer store |
| `/src/store/ui/addLocalModelDownloadDrawer.ts` (36 lines) | `ui/src/modules/llm-provider/llm-model-drawer-store.ts` | NEW - Part of combined drawer store |
| `/src/store/ui/editLocalModelDrawer.ts` (37 lines) | `ui/src/modules/llm-provider/llm-model-drawer-store.ts` | NEW - Part of combined drawer store |
| `/src/store/ui/addRemoteModelDrawer.ts` (44 lines) | `ui/src/modules/llm-provider/llm-model-drawer-store.ts` | NEW - Part of combined drawer store |
| `/src/store/ui/editRemoteModelDrawer.ts` (37 lines) | `ui/src/modules/llm-provider/llm-model-drawer-store.ts` | NEW - Part of combined drawer store |

### **Component Files - Main Drawers**

| react-test Source | ziee-chat Destination | Notes |
|-------------------|----------------------|-------|
| `/src/components/Pages/Settings/Providers/AddLocalModelUploadDrawer.tsx` (437 lines) | `ui/src/modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer.tsx` | NEW - Upload from disk |
| `/src/components/Pages/Settings/Providers/AddLocalModelDownloadDrawer.tsx` (402 lines) | `ui/src/modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx` | NEW - Download from repo |
| `/src/components/Pages/Settings/Providers/EditLocalModelDrawer.tsx` (108 lines) | `ui/src/modules/llm-provider/components/llm-models/EditLlmModelDrawer.tsx` | NEW - Edit LLM model (local) |
| `/src/components/Pages/Settings/Providers/AddRemoteModelDrawer.tsx` (105 lines) | `ui/src/modules/llm-provider/components/llm-models/AddRemoteLlmModelDrawer.tsx` | NEW - Add remote LLM model |
| `/src/components/Pages/Settings/Providers/EditRemoteModelDrawer.tsx` (96 lines) | `ui/src/modules/llm-provider/components/llm-models/EditLlmModelDrawer.tsx` | NEW - Edit LLM model (remote) - merge with local |
| `/src/components/Pages/Settings/Providers/common/ModelsSection.tsx` (365 lines) | `ui/src/modules/llm-provider/components/LocalProviderSettings.tsx` + `RemoteProviderSettings.tsx` | UPDATE - Integrate LLM models table into existing pages |

### **Component Files - Shared Form Sections**

| react-test Source | ziee-chat Destination | Notes |
|-------------------|----------------------|-------|
| `/src/components/Pages/Settings/Providers/common/LocalModelCommonFields.tsx` (69 lines) | `ui/src/modules/llm-provider/components/llm-models/shared/LocalLlmModelCommonFields.tsx` | NEW - Shared form fields |
| `/src/components/Pages/Settings/Providers/common/ModelCapabilitiesSection.tsx` (130 lines) | `ui/src/modules/llm-provider/components/llm-models/shared/LlmModelCapabilitiesSection.tsx` | NEW - Capabilities form |
| `/src/components/Pages/Settings/Providers/common/ModelParametersSection.tsx` (26 lines) | `ui/src/modules/llm-provider/components/llm-models/shared/LlmModelParametersSection.tsx` | NEW - Parameters form |
| `/src/components/Pages/Settings/Providers/common/LlamaCppModelSettingsSection.tsx` (491 lines) | `ui/src/modules/llm-provider/components/llm-models/shared/LlamaCppLlmModelSettingsSection.tsx` | NEW - LlamaCpp settings |
| `/src/components/Pages/Settings/Providers/common/MistralRsModelSettingsSection.tsx` (431 lines) | `ui/src/modules/llm-provider/components/llm-models/shared/MistralRsLlmModelSettingsSection.tsx` | NEW - MistralRs settings |
| `/src/components/Pages/Settings/Providers/common/DeviceSelectionSection.tsx` (~200 lines) | `ui/src/modules/llm-provider/components/llm-models/shared/DeviceSelectionSection.tsx` | NEW - Device selection |
| `/src/components/Pages/Settings/Providers/common/EngineSelectionSection.tsx` (133 lines) | `ui/src/modules/llm-provider/components/llm-models/shared/EngineSelectionSection.tsx` | NEW - Engine selection |

### **Component Files - Utilities**

| react-test Source | ziee-chat Destination | Notes |
|-------------------|----------------------|-------|
| `/src/components/common/ModelParameterField.tsx` (142 lines) | `ui/src/components/common/LlmModelParameterField.tsx` | NEW - Shared utility component |

### **Constants & Configuration**

| react-test Source | ziee-chat Destination | Notes |
|-------------------|----------------------|-------|
| `/src/constants/modelParameters.ts` (145 lines) | `ui/src/modules/llm-provider/constants/llmModelParameters.ts` | NEW - Parameter configs |
| `/src/constants/localModelTypes.ts` (41 lines) | `ui/src/modules/llm-provider/constants/localLlmModelTypes.ts` | NEW - File type definitions |

### **NOT MIGRATING (Hub/API Proxy Components)**

The following components are **NOT** being migrated in this phase as they relate to different features:

- Hub Components (`ModelsTab.tsx`, `ModelCard.tsx`, `ModelDetailsDrawer.tsx`) - Future feature
- API Proxy Components (`ModelSelectionCard.tsx`, `AddModelDrawer.tsx`, `EditModelDrawer.tsx`) - Separate module

---

## Architecture Decision

**LLM Models** are tightly coupled to **LLM Providers** (each model belongs to a provider), so models will be managed **within** the `llm-provider` module, not as a separate module.

**Final File Structure:**
```
ui/src/modules/llm-provider/
├── module.tsx                          # Existing - Module definition
├── store.ts                            # UPDATE - Add LLM model operations export
├── llm-model-download-store.ts         # NEW - From modelDownload.ts
├── drawer-store.ts                     # Existing - Provider drawer state
├── llm-model-drawer-store.ts           # NEW - From 5 drawer stores combined
├── types.ts                            # Existing - Module types
├── constants.tsx                       # Existing - Icons, etc.
├── constants/
│   ├── llmModelParameters.ts           # NEW - From constants/modelParameters.ts
│   └── localLlmModelTypes.ts           # NEW - From constants/localModelTypes.ts
├── components/
│   ├── LlmProviderSettings.tsx         # Existing - Main settings page
│   ├── LocalProviderSettings.tsx       # UPDATE - Add LLM models table (from ModelsSection.tsx)
│   ├── RemoteProviderSettings.tsx      # UPDATE - Add LLM models table (from ModelsSection.tsx)
│   ├── LlmProviderDrawer.tsx           # Existing - Provider create/edit
│   └── llm-models/                     # NEW directory
│       ├── AddLocalLlmModelUploadDrawer.tsx     # NEW - From AddLocalModelUploadDrawer.tsx
│       ├── AddLocalLlmModelDownloadDrawer.tsx   # NEW - From AddLocalModelDownloadDrawer.tsx
│       ├── EditLlmModelDrawer.tsx               # NEW - From Edit*ModelDrawer.tsx (merged)
│       ├── AddRemoteLlmModelDrawer.tsx          # NEW - From AddRemoteModelDrawer.tsx
│       └── shared/                              # NEW directory
│           ├── LocalLlmModelCommonFields.tsx           # NEW
│           ├── LlmModelCapabilitiesSection.tsx         # NEW
│           ├── LlmModelParametersSection.tsx           # NEW
│           ├── LlamaCppLlmModelSettingsSection.tsx     # NEW
│           ├── MistralRsLlmModelSettingsSection.tsx    # NEW
│           ├── DeviceSelectionSection.tsx              # NEW
│           └── EngineSelectionSection.tsx              # NEW

ui/src/components/common/
└── LlmModelParameterField.tsx          # NEW - From common/ModelParameterField.tsx
```

---

## Migration Instructions

### Key Adaptation Rules:

1. **API Client Migration**: Replace all `axios` calls with `ApiClient` from `@/api-client`
   - `axios.get('/api/llm-models')` → `ApiClient.LlmModel.list()`
   - `axios.post('/api/llm-models', data)` → `ApiClient.LlmModel.create(data)`
   - `axios.put('/api/llm-models/${id}', data)` → `ApiClient.LlmModel.update(id, data)`
   - `axios.delete('/api/llm-models/${id}')` → `ApiClient.LlmModel.delete(id)`

2. **Import Path Migration**:
   - `@/types/api` → `@/api-client/types`
   - `@/store/admin/providers` → `@/modules/llm-provider/store`
   - `@/store/ui/*Drawer` → `@/modules/llm-provider/llm-model-drawer-store`
   - `@/constants/modelParameters` → `@/modules/llm-provider/constants/llmModelParameters`
   - `@/constants/localModelTypes` → `@/modules/llm-provider/constants/localLlmModelTypes`

3. **Component Import Migration**:
   - `@/components/Pages/Settings/Providers/common/*` → `@/modules/llm-provider/components/llm-models/shared/*`
   - `@/components/common/ModelParameterField` → `@/components/common/LlmModelParameterField`

4. **Store Pattern**: Use Zustand with `subscribeWithSelector` middleware (follow existing `store.ts` pattern)

5. **Drawer Pattern**: Follow existing `drawer-store.ts` pattern (simple stores with open/close functions)

---

## Implementation Order

### **Phase 1-9: Constants & Utilities** (Foundation)
1. ⬜ Copy `/src/constants/modelParameters.ts` → `constants/llmModelParameters.ts`
2. ⬜ Copy `/src/constants/localModelTypes.ts` → `constants/localLlmModelTypes.ts`
3. ⬜ Copy `/src/components/common/ModelParameterField.tsx` → `ui/src/components/common/LlmModelParameterField.tsx`

### **Phase 10: Download Tracking Store**
4. ⬜ Adapt `/src/store/admin/modelDownload.ts` → `llm-model-download-store.ts`
   - Change axios to ApiClient
   - Update SSE connection to use ziee-chat patterns

### **Phase 11: Drawer Stores** (Combine 5 stores into one)
5. ⬜ Create `llm-model-drawer-store.ts` combining:
   - `addLocalModelUploadDrawer.ts`
   - `addLocalModelDownloadDrawer.ts`
   - `editLocalModelDrawer.ts`
   - `addRemoteModelDrawer.ts`
   - `editRemoteModelDrawer.ts`

### **Phase 12: Shared Form Sections** (7 components)
6. ⬜ Copy `/src/components/Pages/Settings/Providers/common/LocalModelCommonFields.tsx` → `components/llm-models/shared/LocalLlmModelCommonFields.tsx`
7. ⬜ Copy `/src/components/Pages/Settings/Providers/common/ModelCapabilitiesSection.tsx` → `components/llm-models/shared/LlmModelCapabilitiesSection.tsx`
8. ⬜ Copy `/src/components/Pages/Settings/Providers/common/ModelParametersSection.tsx` → `components/llm-models/shared/LlmModelParametersSection.tsx`
9. ⬜ Copy `/src/components/Pages/Settings/Providers/common/LlamaCppModelSettingsSection.tsx` → `components/llm-models/shared/LlamaCppLlmModelSettingsSection.tsx`
10. ⬜ Copy `/src/components/Pages/Settings/Providers/common/MistralRsModelSettingsSection.tsx` → `components/llm-models/shared/MistralRsLlmModelSettingsSection.tsx`
11. ⬜ Copy `/src/components/Pages/Settings/Providers/common/DeviceSelectionSection.tsx` → `components/llm-models/shared/DeviceSelectionSection.tsx`
12. ⬜ Copy `/src/components/Pages/Settings/Providers/common/EngineSelectionSection.tsx` → `components/llm-models/shared/EngineSelectionSection.tsx`

### **Phase 13: Main Drawer Components** (4 drawers)
13. ⬜ Copy `/src/components/Pages/Settings/Providers/AddLocalModelUploadDrawer.tsx` → `components/llm-models/AddLocalLlmModelUploadDrawer.tsx`
14. ⬜ Copy `/src/components/Pages/Settings/Providers/AddLocalModelDownloadDrawer.tsx` → `components/llm-models/AddLocalLlmModelDownloadDrawer.tsx`
15. ⬜ Merge `EditLocalModelDrawer.tsx` + `EditRemoteModelDrawer.tsx` → `components/llm-models/EditLlmModelDrawer.tsx`
16. ⬜ Copy `/src/components/Pages/Settings/Providers/AddRemoteModelDrawer.tsx` → `components/llm-models/AddRemoteLlmModelDrawer.tsx`

### **Phase 14: Integrate into Provider Settings** (2 pages)
17. ⬜ Update `LocalProviderSettings.tsx` - Add LLM models table from `ModelsSection.tsx`
18. ⬜ Update `RemoteProviderSettings.tsx` - Add LLM models table from `ModelsSection.tsx`

### **Phase 15: Store Integration**
19. ⬜ Update `store.ts` - Add LLM model operations from `providers.ts`
20. ⬜ Update `module.tsx` - Register new LLM model stores if needed

---

## TypeScript Type Safety

**Use auto-generated types from `@/api-client/types`:**
- ✅ `LlmModel` - Model data type
- ✅ `CreateLlmModelRequest` - Create request
- ✅ `UpdateLlmModelRequest` - Update request
- ✅ `DownloadFromRepositoryRequest` - Download request
- ✅ `DownloadInstance` - Download status
- ✅ `LlmModelListResponse` - List response

**Never:**
- ❌ Define duplicate types
- ❌ Use `any` without `TODO` comment
- ❌ Import from `axios` (doesn't exist)

---

## Backend Limitations (As of Implementation)

**Missing Backend Features (marked with TODO in frontend):**
1. SSE endpoint for download progress - `GET /api/llm-models/downloads/progress`
2. Cancel download endpoint - `DELETE /api/llm-models/downloads/{id}/cancel`
3. Delete download endpoint - `DELETE /api/llm-models/downloads/{id}`
4. List downloads endpoint - `GET /api/llm-models/downloads`

**Workaround:** Frontend includes stub implementations with console warnings. When backend implements these, remove TODOs and enable functionality.

---

## Testing Checklist

After implementation:
- [ ] TypeScript compiles with no errors (`npx tsc --noEmit`)
- [ ] Upload local model works
- [ ] Download from repository initiates (progress pending backend)
- [ ] Edit model settings saves correctly
- [ ] Enable/disable model works
- [ ] Delete model works
- [ ] Models table displays correctly for local providers
- [ ] Models table displays correctly for remote providers
- [ ] Drawers open/close correctly
- [ ] Form validation works
- [ ] Upload progress displays
- [ ] Error messages display

---

## Notes

- **Module-First Approach:** Models are part of llm-provider module, not separate
- **Store Pattern:** Follow existing Zustand + `__init__` + `Stores` pattern
- **No Axios:** All API calls via `ApiClient`
- **Component Patterns:** Follow existing Ant Design component usage
- **Drawer Patterns:** Follow existing drawer-store pattern
- **Type Safety:** Use auto-generated OpenAPI types exclusively
