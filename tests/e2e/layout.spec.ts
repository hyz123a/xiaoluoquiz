import { expect, test } from '@playwright/test';

import { loginAsAdmin } from './helpers';

async function assertNoHorizontalOverflow(page: import('@playwright/test').Page) {
  const dimensions = await page.evaluate(() => ({
    documentWidth: document.documentElement.scrollWidth,
    viewportWidth: document.documentElement.clientWidth,
  }));
  expect(dimensions.documentWidth).toBeLessThanOrEqual(dimensions.viewportWidth);
}

test.describe('响应式应用壳', () => {
  test('桌面端显示完整导航并可切换到账号管理', async ({ page }) => {
    await page.setViewportSize({ width: 1440, height: 900 });
    await loginAsAdmin(page, '/admin');

    await expect(page.getByTestId('desktop-app-nav')).toBeVisible();
    await expect(page.getByTestId('mobile-nav-toggle')).toBeHidden();
    await page.getByTestId('desktop-app-nav').getByRole('link', { name: '账号管理' }).click();
    await expect(page).toHaveURL(/\/admin\/users$/);
    await expect(page.getByTestId('admin-users-page')).toBeVisible();
    await assertNoHorizontalOverflow(page);
  });

  test('iPhone 上使用 Dracula 主题并通过折叠菜单访问后台导航', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await loginAsAdmin(page, '/admin');

    await expect(page.locator('html')).toHaveAttribute('data-theme', 'dracula');
    await expect(page.getByTestId('app-shell')).toBeVisible();
    await expect(page.getByTestId('mobile-nav-toggle')).toBeVisible();
    await page.getByTestId('mobile-nav-toggle').click();
    await expect(page.getByTestId('app-nav-menu')).toBeVisible();
    const usersLink = page.getByTestId('app-nav-menu').getByRole('link', { name: '账号管理' });
    await expect(usersLink).toBeVisible();
    await usersLink.click();
    await expect(page).toHaveURL(/\/admin\/users$/);
    await expect(page.getByTestId('admin-users-page')).toBeVisible();
    await assertNoHorizontalOverflow(page);
  });

  test('iPad 竖屏上所有共享应用壳的页面保持在视口内', async ({ page }) => {
    await page.setViewportSize({ width: 768, height: 1024 });
    await loginAsAdmin(page, '/admin');

    const routes = [
      ['/', 'practice-page'],
      ['/papers', 'paper-list'],
      ['/admin', 'admin-page'],
      ['/admin/papers', 'admin-papers-page'],
      ['/admin/users', 'admin-users-page'],
    ] as const;

    for (const [route, testId] of routes) {
      await page.goto(route);
      await expect(page.getByTestId(testId)).toBeVisible();
      await expect(page.getByTestId('mobile-nav-toggle')).toBeVisible();
      await assertNoHorizontalOverflow(page);

      const shell = await page.getByTestId('app-shell').boundingBox();
      expect(shell).not.toBeNull();
      expect(shell?.width).toBeLessThanOrEqual(768);
    }
  });
});
