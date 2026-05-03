/**
 * Routes that are valid even when the user has no `current_org` selected
 * (zero-Org state, or just hasn't picked an Org yet). Everything else
 * implicitly requires `current_org`.
 */
const ORG_AGNOSTIC_PATHS = new Set([
  '/no-org',
  '/orgs/new',
  '/orgs/join',
])

export default defineNuxtRouteMiddleware(async (to) => {
  const auth = useAuth()
  await auth.ensureLoaded()
  if (!auth.isAuthenticated.value) {
    return navigateTo({ path: '/login', query: { next: to.fullPath } })
  }
  if (auth.currentOrg.value === null && !ORG_AGNOSTIC_PATHS.has(to.path)) {
    return navigateTo('/no-org')
  }
})
