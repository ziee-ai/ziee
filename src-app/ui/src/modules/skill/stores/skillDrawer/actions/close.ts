import type { SkillDrawerSet } from '../state'

export default (set: SkillDrawerSet) => async () => {
  set(d => {
    d.isOpen = false
  })
}
