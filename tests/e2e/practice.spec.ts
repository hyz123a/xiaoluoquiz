import { expect, test, type Page } from '@playwright/test';

import { expandPracticeQuestionNavigation, loginAsAdmin, openPracticeQuestion } from './helpers';

const SINGLE_STEM = 'Rust 的包管理工具是什么？';
const MULTIPLE_STEM = '以下哪些是 Rust 开发工具？';
const FILL_BLANK_STEM = 'Rust 的异步运行时通常使用 ___。';
const TRUE_FALSE_STEM = 'PostgreSQL 支持 JSONB 类型。';
const SHORT_ANSWER_STEM = '请用一句话说明 SQLx 的作用。';

function questionCard(page: Page, stem: string) {
  return page.locator('article[data-testid^="question-"]').filter({ hasText: stem });
}

test.describe('练习页面', () => {
  test('默认显示全部题库和题型', async ({ page }) => {
    await loginAsAdmin(page);

    await expect(page.getByTestId('question-list')).toBeVisible();
    await expect(page.getByTestId('question-bank-filter')).toHaveValue('');
    await expect(page.getByTestId('question-filter')).toHaveValue('all');
    await expect(page.getByTestId('practice-question-numbers')).toBeVisible();
    await expandPracticeQuestionNavigation(page);

    const expectedStems = [
      SINGLE_STEM,
      FILL_BLANK_STEM,
      TRUE_FALSE_STEM,
      SHORT_ANSWER_STEM,
      MULTIPLE_STEM,
    ];
    for (const [index, stem] of expectedStems.entries()) {
      await page.getByTestId(`practice-question-number-${index}`).click();
      await expect(page.getByTestId('question-list').locator('article[data-testid^="question-"]')).toHaveCount(1);
      await expect(page.getByTestId('question-list')).toContainText(stem);
    }
  });

  test('题目导航默认收起，并可以展开和收起', async ({ page }) => {
    await loginAsAdmin(page);

    const toggle = page.getByTestId('practice-question-numbers-toggle');
    const panel = page.getByTestId('practice-question-navigation-panel');
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');
    await expect(panel).toBeHidden();

    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-expanded', 'true');
    await expect(panel).toBeVisible();
    await expect(page.getByTestId('practice-question-number-0')).toBeVisible();

    await toggle.click();
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');
    await expect(panel).toBeHidden();
  });

  test('学生可以按题库筛选题目', async ({ page }) => {
    await loginAsAdmin(page);

    const bankFilter = page.getByTestId('question-bank-filter');
    await expect(bankFilter).toBeVisible();
    for (const bankName of ['人工智能导论', '测试题库']) {
      await expect(bankFilter.locator('option', { hasText: bankName })).toHaveCount(1);
    }

    await bankFilter.selectOption({ label: '人工智能导论' });
    await expect(bankFilter).toHaveValue('1');
    await expect(page.getByTestId('question-list')).toBeVisible();
    await expect(page.getByTestId('empty-state')).toHaveCount(0);
    await expect(page.getByTestId('question-list').locator('article[data-testid^="question-"]')).toHaveCount(1);

    await bankFilter.selectOption({ label: '测试题库' });
    await expect(bankFilter).toHaveValue('2');
    await expect(page.getByTestId('question-list')).toBeVisible();
    await expect(page.getByText(SINGLE_STEM)).toBeVisible();
  });

  test('学生可以按题型筛选并提交选择题', async ({ page }) => {
    await loginAsAdmin(page);

    await expect(page.getByTestId('question-list')).toBeVisible();
    await expect(page.getByText(SINGLE_STEM)).toBeVisible();
    await expandPracticeQuestionNavigation(page);
    await page.getByTestId('practice-question-number-1').click();
    await expect(page.getByText(FILL_BLANK_STEM)).toBeVisible();

    await page.getByTestId('question-filter').selectOption('single_choice');
    await expect(page.getByText(SINGLE_STEM)).toBeVisible();
    await expect(page.getByText(FILL_BLANK_STEM)).toHaveCount(0);

    const card = questionCard(page, SINGLE_STEM);
    await card.getByLabel(/B．Cargo/).check();
    await card.getByTestId('submit-answer').click();

    await expect(card.getByTestId('answer-result')).toContainText('回答正确');
    await expect(card.getByTestId('answer-result')).toContainText('服务端判断：回答正确');
    await expect(card.getByTestId('answer-result')).toContainText('B');
  });

  test('学生可以按任意顺序选择多选题的多个选项并提交', async ({ page }) => {
    await loginAsAdmin(page);

    await page.getByTestId('question-filter').selectOption('multiple_choice');
    const card = questionCard(page, MULTIPLE_STEM);
    await expect(card).toBeVisible();
    await expect(card.getByLabel(/A．Cargo/)).toHaveCount(1);
    await expect(card.getByLabel(/E．Clippy/)).toHaveCount(1);

    await card.getByLabel(/E．Clippy/).check();
    await card.getByLabel(/A．Cargo/).check();
    await card.getByLabel(/C．rustc/).check();
    await expect(card.getByLabel(/B．npm/)).not.toBeChecked();

    const requestPromise = page.waitForRequest(
      (request) =>
        request.method() === 'POST' &&
        request.url().includes('/api/v1/questions/') &&
        request.url().endsWith('/check'),
    );
    await card.getByTestId('submit-answer').click();
    const request = await requestPromise;
    expect(request.postDataJSON()).toMatchObject({
      answer: { type: 'multiple_choice', option_keys: ['E', 'A', 'C'] },
    });
    await expect(card.getByTestId('answer-result')).toContainText('回答正确');
    await expect(card.getByTestId('answer-result')).toContainText('A、C、E');
  });

  test('跳转题目时不会保留上一题的判题解析', async ({ page }) => {
    await loginAsAdmin(page);

    const single = questionCard(page, SINGLE_STEM);
    await single.getByLabel(/B．Cargo/).check();
    await single.getByTestId('submit-answer').click();
    await expect(single.getByTestId('answer-result')).toContainText('回答正确');

    await expandPracticeQuestionNavigation(page);
    await page.getByTestId('practice-question-number-1').click();
    await expect(questionCard(page, FILL_BLANK_STEM)).toBeVisible();
    await expect(page.getByTestId('answer-result')).toHaveCount(0);
  });

  test('用户可以提交填空题、判断题和简答题', async ({ page }) => {
    await loginAsAdmin(page);

    await expandPracticeQuestionNavigation(page);
    await page.getByTestId('practice-question-number-1').click();
    const fillBlank = questionCard(page, FILL_BLANK_STEM);
    await fillBlank.locator('input').fill('Tokio');
    await fillBlank.getByTestId('submit-answer').click();
    await expect(fillBlank.getByTestId('answer-result')).toContainText('回答正确');

    await page.getByTestId('practice-question-number-2').click();
    const trueFalse = questionCard(page, TRUE_FALSE_STEM);
    await trueFalse.getByLabel('正确', { exact: true }).check();
    await trueFalse.getByTestId('submit-answer').click();
    await expect(trueFalse.getByTestId('answer-result')).toContainText('回答正确');

    await page.getByTestId('practice-question-number-3').click();
    const shortAnswer = questionCard(page, SHORT_ANSWER_STEM);
    await shortAnswer.locator('textarea').fill('SQLx 提供 Rust 到数据库的异步访问。');
    await shortAnswer.getByTestId('submit-answer').click();
    await expect(shortAnswer.getByTestId('answer-result')).toContainText('等待批改');
    await expect(shortAnswer.getByTestId('answer-result')).toContainText(
      'SQLx 为 Rust 提供异步数据库访问能力',
    );
  });

  test('表单校验、空列表和加载失败状态可见', async ({ page }) => {
    await loginAsAdmin(page);

    const single = questionCard(page, SINGLE_STEM);
    await single.getByTestId('submit-answer').click();
    await expect(single.getByRole('alert')).toContainText('请选择一个选项');

    await page.route('**/api/v1/questions', async (route) => {
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ items: [] }),
      });
    });
    await page.reload();
    await expect(page.getByTestId('empty-state')).toBeVisible();

    await page.unroute('**/api/v1/questions');
    await page.route('**/api/v1/questions', async (route) => {
      await route.abort('failed');
    });
    await page.reload();
    await expect(page.getByTestId('error-state')).toBeVisible();
  });

  test('答题卡片实时显示整体正确率，并排除简答题和重复作答', async ({ page }) => {
    await loginAsAdmin(page);
    await page.evaluate(() => localStorage.clear());

    const accuracy = page.getByTestId('practice-accuracy');
    await expect(accuracy).toBeVisible();
    await expect(accuracy).toHaveAttribute('data-answered-count', /^\d+$/);
    await expect(accuracy).toHaveAttribute('data-correct-count', /^\d+$/);

    await openPracticeQuestion(page, MULTIPLE_STEM);
    const multiple = questionCard(page, MULTIPLE_STEM);
    await multiple.getByLabel(/A．Cargo/).check();
    await multiple.getByLabel(/C．rustc/).check();
    await multiple.getByLabel(/E．Clippy/).check();

    const firstResponsePromise = page.waitForResponse(
      (response) =>
        response.request().method() === 'POST' &&
        response.url().includes('/api/v1/questions/') &&
        response.url().endsWith('/check'),
    );
    await multiple.getByTestId('submit-answer').click();
    const firstResponse = await firstResponsePromise;
    expect(firstResponse.ok()).toBe(true);
    const firstBody = await firstResponse.json();
    await expect(accuracy).toHaveAttribute(
      'data-answered-count',
      String(firstBody.practice_stats.answered_count),
    );
    await expect(accuracy).toHaveAttribute(
      'data-correct-count',
      String(firstBody.practice_stats.correct_count),
    );

    await multiple.getByLabel(/B．npm/).check();
    const repeatedResponsePromise = page.waitForResponse(
      (response) =>
        response.request().method() === 'POST' &&
        response.url().includes('/api/v1/questions/') &&
        response.url().endsWith('/check'),
    );
    await multiple.getByTestId('submit-answer').click();
    const repeatedResponse = await repeatedResponsePromise;
    expect(repeatedResponse.ok()).toBe(true);
    const repeatedBody = await repeatedResponse.json();
    expect(repeatedBody.practice_stats).toEqual(firstBody.practice_stats);
    await expect(accuracy).toHaveAttribute(
      'data-answered-count',
      String(firstBody.practice_stats.answered_count),
    );
    await expect(accuracy).toHaveAttribute(
      'data-correct-count',
      String(firstBody.practice_stats.correct_count),
    );

    await openPracticeQuestion(page, SHORT_ANSWER_STEM);
    const shortAnswer = questionCard(page, SHORT_ANSWER_STEM);
    await shortAnswer.locator('textarea').fill('SQLx 提供 Rust 到数据库的异步访问。');
    const shortResponsePromise = page.waitForResponse(
      (response) =>
        response.request().method() === 'POST' &&
        response.url().includes('/api/v1/questions/') &&
        response.url().endsWith('/check'),
    );
    await shortAnswer.getByTestId('submit-answer').click();
    const shortResponse = await shortResponsePromise;
    expect(shortResponse.ok()).toBe(true);
    const shortBody = await shortResponse.json();
    expect(shortBody.status).toBe('needs_review');
    expect(shortBody.practice_stats).toEqual(firstBody.practice_stats);
    await expect(accuracy).toHaveAttribute(
      'data-answered-count',
      String(firstBody.practice_stats.answered_count),
    );
    await expect(accuracy).toHaveAttribute(
      'data-correct-count',
      String(firstBody.practice_stats.correct_count),
    );

    await page.reload();
    await expect(page.getByTestId('question-list')).toBeVisible();
    await expect(accuracy).toHaveAttribute(
      'data-answered-count',
      String(firstBody.practice_stats.answered_count),
    );
    await expect(accuracy).toHaveAttribute(
      'data-correct-count',
      String(firstBody.practice_stats.correct_count),
    );
  });

  test('移动端题目页面没有横向溢出', async ({ page }) => {
    await page.setViewportSize({ width: 390, height: 844 });
    await loginAsAdmin(page);

    await expect(page.getByTestId('question-list')).toBeVisible();
    const hasHorizontalOverflow = await page.evaluate(
      () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
    );
    expect(hasHorizontalOverflow).toBe(false);
  });

  test('练习按单题卡片展示，支持顺序导航、题号跳转和非简答题答案恢复', async ({ page }) => {
    await loginAsAdmin(page);
    await page.evaluate(() => localStorage.clear());

    await page.getByTestId('question-bank-filter').selectOption({ label: '测试题库' });
    await expandPracticeQuestionNavigation(page);
    await expect(page.getByTestId('practice-question-numbers')).toBeVisible();
    await expect(page.getByTestId('practice-question-number-0')).toHaveCount(1);
    await expect(page.getByTestId('practice-question-number-4')).toHaveCount(1);
    await expect(page.getByTestId('question-list').locator('article[data-testid^="question-"]')).toHaveCount(1);

    const first = page.locator('article[data-testid^="question-"]').first();
    await expect(first).toContainText(SINGLE_STEM);
    await first.getByLabel(/B．Cargo/).check();
    await expect(page.getByTestId('practice-question-number-0')).toHaveAttribute('data-answered', 'true');

    await page.getByTestId('practice-next').click();
    await expect(page.locator('article[data-testid^="question-"]').first()).toContainText(FILL_BLANK_STEM);
    await expect(page.locator('article[data-testid^="question-"]')).toHaveCount(1);

    await page.getByTestId('practice-question-number-0').click();
    const returned = page.locator('article[data-testid^="question-"]').first();
    await expect(returned).toContainText(SINGLE_STEM);
    await expect(returned.getByLabel(/B．Cargo/)).toBeChecked();

    await page.reload();
    await expect(page.getByTestId('question-list')).toBeVisible();
    await page.getByTestId('question-bank-filter').selectOption({ label: '测试题库' });
    const reloaded = page.locator('article[data-testid^="question-"]').first();
    await expect(reloaded).toContainText(SINGLE_STEM);
    await expect(reloaded.getByLabel(/B．Cargo/)).toBeChecked();
  });
});
