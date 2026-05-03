export default defineNuxtRouteMiddleware(async (to) => {
  const auth = useAuth()
  await auth.ensureLoaded()
  if (!auth.isAuthenticated.value) {
    return navigateTo({ path: '/login', query: { next: to.fullPath } })
  }
})
