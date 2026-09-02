import { expect, test } from '@playwright/test';

import {
  INITIAL_PASSWORD,
  loginAsAdmin,
} from './helpers';

test('登录失败会显示错误，管理员可以创建账号并完成首次改密', async ({ page }, testInfo) => {
  await page.goto('/login');
  await page.getByTestId('login-username').fill('unknown-user');
  await page.getByTestId('login-password').fill('wrong-password');
  await page.getByTestId('login-submit').click();
  await expect(page.getByTestId('login-error')).toContainText('账号或密码错误');

  const username = `e2e-student-${testInfo.project.name}-${Date.now()}`;
  await loginAsAdmin(page, '/admin/users');
  await expect(page.getByTestId('admin-users-page')).toBeVisible();
  await page.getByTestId('admin-user-username').fill(username);
  await page.getByTestId('admin-user-display-name').fill('E2E 学生');
  await page.getByTestId('admin-user-student-number').fill('20269999');
  const className = `软件工程测试班-${testInfo.project.name}-${Date.now()}`;
  await page.getByTestId('admin-class-name').fill(className);
  await page.getByTestId('admin-class-save').click();
  await expect(page.getByTestId('admin-user-class').locator('option').filter({ hasText: className })).toHaveCount(1);
  await page.getByTestId('admin-user-class').selectOption({ label: className });
  await page.getByTestId('admin-user-save').click();

  const row = page.getByTestId('admin-user-row').filter({ hasText: username });
  await expect(row).toContainText('E2E 学生');
  await expect(row).toContainText('首次登录');
  await expect(row.locator('select')).toHaveValue('student');
  await expect(row).toContainText(className);

  await page.getByTestId('logout').click();
  await expect(page.getByTestId('login-page')).toBeVisible();
  await page.getByTestId('login-username').fill(username);
  await page.getByTestId('login-password').fill(INITIAL_PASSWORD);
  await page.getByTestId('login-submit').click();
  await expect(page.getByTestId('change-password-page')).toBeVisible();

  await page.getByTestId('change-password-new').fill('StudentPassword123!');
  await page.getByTestId('change-password-confirm').fill('StudentPassword123!');
  await page.getByTestId('change-password-submit').click();
  await expect(page.getByTestId('question-list')).toBeVisible();
});

test('登录页提示学生账号默认密码', async ({ page }) => {
  await page.goto('/login');
  await expect(page.getByTestId('login-page')).toBeVisible();
  await expect(page.getByTestId('login-default-password')).toContainText(INITIAL_PASSWORD);
  await expect(page.getByTestId('login-default-password')).toContainText('首次登录后必须修改');
});

test('登录页支持显示密码，并保持账号和密码输入框对齐', async ({ page }) => {
  await page.goto('/login');
  await expect(page.getByTestId('login-page')).toBeVisible();

  const usernameInput = page.getByTestId('login-username');
  const passwordInput = page.getByTestId('login-password');
  const togglePassword = page.getByTestId('toggle-password-visibility');

  await expect(togglePassword).toBeVisible();
  await expect(passwordInput).toHaveAttribute('type', 'password');
  await togglePassword.click();
  await expect(passwordInput).toHaveAttribute('type', 'text');
  await expect(togglePassword).toContainText('隐藏');
  await togglePassword.click();
  await expect(passwordInput).toHaveAttribute('type', 'password');
  await expect(togglePassword).toContainText('显示');

  const [usernameRect, passwordRect] = await Promise.all(
    [usernameInput, passwordInput].map((input) =>
      input.evaluate((element) => {
        const rect = element.getBoundingClientRect();
        return { x: rect.x, width: rect.width };
      }),
    ),
  );
  expect(Math.abs(usernameRect.x - passwordRect.x)).toBeLessThan(1);
  expect(Math.abs(usernameRect.width - passwordRect.width)).toBeLessThan(1);
});
