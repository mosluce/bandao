export default defineNuxtRouteMiddleware(async () => {
  const auth = useAuth()
  await auth.ensureLoaded()
  if (auth.isAuthenticated.value) {
    return navigateTo('/')
  }
})
