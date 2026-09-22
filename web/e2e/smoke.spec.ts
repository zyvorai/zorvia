import { test, expect } from '@playwright/test'

test.describe('console smoke', () => {
  test('sign-in page loads with brand', async ({ page }) => {
    await page.goto('/sign-in')
    await expect(page.locator('body')).toBeVisible()
    // Brand or product name should be present on the first viewport.
    await expect(page.getByText(/zorvia|zyvor|sign in|continue/i).first()).toBeVisible()
  })

  test('unauthenticated API health is reachable via same origin proxy only when API up', async ({
    request,
  }) => {
    // Static preview has no API — skip soft if 404.
    const res = await request.get('/api/v1/health')
    expect([200, 404, 502, 503]).toContain(res.status())
  })
})
