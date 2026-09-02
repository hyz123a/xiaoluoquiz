import { expect, type Page } from '@playwright/test';

export const ADMIN_USERNAME = process.env.E2E_ADMIN_USERNAME ?? 'demo-admin';
export const INITIAL_PASSWORD = process.env.INITIAL_PASSWORD ?? 'InitialPassword123!';
export const UPDATED_ADMIN_PASSWORD =
  process.env.E2E_ADMIN_PASSWORD ?? 'AdminPassword123!';

async function submitLogin(page: Page, password: string) {
  await page.getByTestId('login-username').fill(ADMIN_USERNAME);
  await page.getByTestId('login-password').fill(password);
  const responsePromise = page.waitForResponse(
    (response) => response.url().includes('/api/v1/auth/login'),
  );
  await page.getByTestId('login-submit').click();
  return responsePromise;
}

export async function loginAsAdmin(page: Page, target = '/') {
  await page.goto(target);
  const appState = page.locator(
    '[data-testid="login-page"], [data-testid="change-password-page"], [data-testid="admin-page"], [data-testid="admin-users-page"], [data-testid="admin-papers-page"], [data-testid="admin-attempt-list"], [data-testid="admin-attempt-detail-page"], [data-testid="question-list"], [data-testid="paper-list"], [data-testid="paper-start-page"], [data-testid="exam-page"], [data-testid="exam-result-page"], [data-testid="auth-error"]',
  ).first();
  await expect(appState).toBeVisible();

  const loginPage = page.getByTestId('login-page');
  if (await loginPage.isVisible()) {
    let response = await submitLogin(page, UPDATED_ADMIN_PASSWORD);
    if (!response.ok()) {
      await expect(page.getByTestId('login-submit')).toBeEnabled();
      response = await submitLogin(page, INITIAL_PASSWORD);
    }
    expect(response.ok()).toBe(true);

    const changePasswordPage = page.getByTestId('change-password-page');
    if (await changePasswordPage.isVisible({ timeout: 3_000 }).catch(() => false)) {
      await page.getByTestId('change-password-new').fill(UPDATED_ADMIN_PASSWORD);
      await page.getByTestId('change-password-confirm').fill(UPDATED_ADMIN_PASSWORD);
      await page.getByTestId('change-password-submit').click();
    }
  }

  if (target.startsWith('/admin/users')) {
    await expect(page.getByTestId('admin-users-page')).toBeVisible();
  } else if (target.startsWith('/admin/papers')) {
    await expect(page.getByTestId('admin-papers-page')).toBeVisible();
  } else if (target.startsWith('/admin/attempts')) {
    await expect(page.getByTestId('admin-attempt-list')).toBeVisible();
  } else if (target.startsWith('/admin')) {
    await expect(page.getByTestId('admin-page')).toBeVisible();
  } else if (target.startsWith('/papers')) {
    await expect(page.getByTestId(target.includes('/start') ? 'paper-start-page' : 'paper-list')).toBeVisible();
  } else if (target.startsWith('/exam')) {
    await expect(page.getByTestId(target.includes('/result') ? 'exam-result-page' : 'exam-page')).toBeVisible();
  } else {
    await expect(page.getByTestId('question-list')).toBeVisible();
  }
}

export async function loginWithCredentials(
  page: Page,
  username: string,
  password: string,
  target = '/',
) {
  await page.goto(target);
  await expect(page.getByTestId('login-page')).toBeVisible();
  await page.getByTestId('login-username').fill(username);
  await page.getByTestId('login-password').fill(password);
  await page.getByTestId('login-submit').click();
}

export async function expandPracticeQuestionNavigation(page: Page) {
  const toggle = page.getByTestId('practice-question-numbers-toggle');
  if ((await toggle.getAttribute('aria-expanded')) !== 'true') {
    await toggle.click();
  }
  await expect(toggle).toHaveAttribute('aria-expanded', 'true');
}

export async function openPracticeQuestion(page: Page, stem: string) {
  await expandPracticeQuestionNavigation(page);
  const questionNumbers = page.locator('button[data-testid^="practice-question-number-"]');
  const target = page.getByTestId('question-list').locator('article').filter({ hasText: stem });

  for (let index = 0; index < (await questionNumbers.count()); index += 1) {
    await questionNumbers.nth(index).click();
    try {
      await expect(target).toBeVisible({ timeout: 500 });
      return;
    } catch {
      // Continue until the target question is displayed.
    }
  }

  await expect(target).toBeVisible();
}

export async function expectPracticeQuestionAbsent(page: Page, stem: string) {
  await expandPracticeQuestionNavigation(page);
  const questionNumbers = page.locator('button[data-testid^="practice-question-number-"]');
  const target = page.getByTestId('question-list').locator('article').filter({ hasText: stem });

  for (let index = 0; index < (await questionNumbers.count()); index += 1) {
    await questionNumbers.nth(index).click();
    await expect(target).toHaveCount(0);
  }
}
