# useDelayedFalse Hook

## Purpose

Delays the transition from `true` to `false`, allowing components to stay mounted for animations/transitions even after the underlying state becomes false.

---

## Signature

```typescript
function useDelayedFalse(
  hookFunction: () => boolean,
  delay?: number // default: 3000ms
): boolean
```

---

## Behavior

| Input Changes | Returned Value | Timing |
|--------------|----------------|--------|
| `false → true` | `true` | Immediately |
| `true → false` | `true` → `false` | Stays `true`, then `false` after delay |
| `false → true` (while waiting) | `true` | Immediately, cancels pending `false` |

---

## Use Cases

### 1. Drawer Exit Animations

Keep drawer mounted while closing animation plays:

```typescript
// module.tsx
components: [
  {
    id: 'my-drawer',
    component: MyDrawer,
    shouldMount: () =>
      useDelayedFalse(() => Stores.MyDrawer.isOpen)
  }
]
```

**Timeline:**
```
t=0s:  isOpen = true  → shouldMount = true  (drawer visible)
t=1s:  isOpen = false → shouldMount = true  (closing animation plays)
t=4s:  isOpen = false → shouldMount = false (drawer unmounts)
```

If user reopens during animation:
```
t=0s:  isOpen = true  → shouldMount = true
t=1s:  isOpen = false → shouldMount = true  (closing animation)
t=2s:  isOpen = true  → shouldMount = true  (cancels delayed false!)
```

### 2. Modal Fade Out

```typescript
shouldMount: () =>
  useDelayedFalse(() => Stores.Modal.visible, 500)
```

### 3. Toast Notifications

```typescript
shouldMount: () =>
  useDelayedFalse(() => Stores.Toast.show, 2000)
```

---

## Example: MCP Drawer

```typescript
// modules/mcp/module.tsx
import { useDelayedFalse } from '@/hooks/useDelayedFalse'

export default createModule({
  components: [
    {
      id: 'mcp-drawer',
      component: McpDrawer,
      shouldMount: () =>
        useDelayedFalse(() => Stores.McpDrawer.isOpen)
      // Drawer stays mounted 3s after closing for animation
    }
  ]
})
```

**What happens:**

1. **Opening drawer:**
   - `Stores.McpDrawer.isOpen` → `true`
   - `shouldMount` → `true` immediately
   - Drawer mounts instantly

2. **Closing drawer:**
   - `Stores.McpDrawer.isOpen` → `false`
   - `shouldMount` → stays `true` for 3 seconds
   - Drawer closing animation plays
   - After 3s, `shouldMount` → `false`
   - Drawer unmounts

3. **Quick reopen (within 3s):**
   - `Stores.McpDrawer.isOpen` → `true` again
   - Delayed false is cancelled
   - `shouldMount` → stays `true`
   - Drawer reopens smoothly

---

## Custom Delay

```typescript
// 500ms delay
shouldMount: () =>
  useDelayedFalse(() => Stores.QuickModal.visible, 500)

// 5 second delay
shouldMount: () =>
  useDelayedFalse(() => Stores.SlowPanel.show, 5000)
```

---

## Implementation Details

```typescript
export function useDelayedFalse(
  hookFunction: () => boolean,
  delay: number = 3000,
): boolean {
  const currentValue = hookFunction()
  const [delayedValue, setDelayedValue] = useState(currentValue)
  const timeoutRef = useRef<NodeJS.Timeout | null>(null)

  useEffect(() => {
    // True → immediately return true and cancel timeout
    if (currentValue === true) {
      if (timeoutRef.current !== null) {
        clearTimeout(timeoutRef.current)
        timeoutRef.current = null
      }
      setDelayedValue(true)
      return
    }

    // False → schedule delayed false
    if (currentValue === false && delayedValue === true) {
      timeoutRef.current = setTimeout(() => {
        setDelayedValue(false)
        timeoutRef.current = null
      }, delay)

      return () => {
        if (timeoutRef.current !== null) {
          clearTimeout(timeoutRef.current)
        }
      }
    }
  }, [currentValue, delayedValue, delay])

  return delayedValue
}
```

---

## Benefits

1. **Clean Animations**: Components stay mounted during exit animations
2. **Automatic Cleanup**: Timeout cancelled if component unmounts
3. **Smart Cancellation**: Reopening cancels the delayed unmount
4. **Flexible**: Configurable delay for different use cases
5. **Type Safe**: Full TypeScript support

---

## Common Patterns

### Pattern 1: Always Delayed False

```typescript
shouldMount: () => useDelayedFalse(() => Stores.MyDrawer.isOpen)
```

### Pattern 2: Conditional Delayed False

```typescript
shouldMount: () => {
  const isOpen = Stores.MyDrawer.isOpen
  const hasAnimation = Stores.Settings.animationsEnabled

  // Only delay if animations are enabled
  if (hasAnimation) {
    return useDelayedFalse(() => isOpen)
  }
  return isOpen
}
```

### Pattern 3: Multiple Conditions

```typescript
shouldMount: () => {
  const isOpen = Stores.MyDrawer.isOpen
  const isAuthenticated = Stores.Auth.isAuthenticated

  // Only mount if authenticated AND (open OR waiting to close)
  return isAuthenticated && useDelayedFalse(() => isOpen)
}
```

---

## Testing

```typescript
// Test in browser console
const store = Stores.MyDrawer

// Open drawer
store.open()
// shouldMount → true immediately

// Close drawer
store.close()
// shouldMount → still true for 3 seconds
// (watch the animation play)

// After 3 seconds
// shouldMount → false (drawer unmounts)

// Quick test: reopen within 3 seconds
store.close()
setTimeout(() => store.open(), 1000)
// shouldMount → stays true (no flicker!)
```

---

## Performance

- ✅ Minimal overhead (single timeout)
- ✅ Automatic cleanup on unmount
- ✅ No memory leaks
- ✅ Efficient re-renders (only when value changes)

---

**Ready to use for smooth component transitions!** 🎉
