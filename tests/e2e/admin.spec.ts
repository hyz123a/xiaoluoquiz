import { expect, test } from '@playwright/test';
import {
  expectPracticeQuestionAbsent,
  loginAsAdmin,
  openPracticeQuestion,
} from './helpers';


test('题型下拉与答案字段保持一致，选择题默认提供 A 到 D 选项', async ({ page }) => {
  await loginAsAdmin(page, '/admin');
  await expect(page.getByTestId('admin-page')).toBeVisible();

  const questionType = page.getByTestId('admin-question-type');
  const questionBank = page.getByTestId('admin-question-bank');
  await expect(questionBank).toBeVisible();
  for (const bankName of ['人工智能导论', '测试题库']) {
    await expect(questionBank.locator('option', { hasText: bankName })).toHaveCount(1);
  }
  await expect(page.getByTestId('admin-score')).toHaveCount(0);
  await expect(questionType).toHaveValue('single_choice');
  await expect(page.getByTestId('admin-option-A')).toHaveCount(1);
  await expect(page.getByTestId('admin-option-B')).toHaveCount(1);
  await expect(page.getByTestId('admin-option-C')).toHaveCount(1);
  await expect(page.getByTestId('admin-option-D')).toHaveCount(1);
  await expect(page.getByTestId('admin-correct-option').locator('option')).toHaveText([
    'A',
    'B',
    'C',
    'D',
  ]);

  await page.getByTestId('admin-correct-option').selectOption('D');
  await expect(page.getByTestId('admin-correct-option')).toHaveValue('D');

  await questionType.selectOption('short_answer');
  await page.getByTestId('admin-stem').fill('下拉映射简答题');
  await expect(questionType).toHaveValue('short_answer');
  await expect(page.getByTestId('admin-reference')).toBeVisible();
  await expect(page.getByTestId('admin-option-A')).toHaveCount(0);

  await questionType.selectOption('true_false');
  await expect(questionType).toHaveValue('true_false');
  await expect(page.getByTestId('admin-true-false-answer')).toBeVisible();
  await expect(page.getByTestId('admin-reference')).toHaveCount(0);

  await questionType.selectOption('fill_blank');
  await expect(questionType).toHaveValue('fill_blank');
  await expect(page.getByTestId('admin-fill-blank-answer')).toBeVisible();
  await expect(page.getByTestId('admin-true-false-answer')).toHaveCount(0);
});


test('选择简答题后提交的题型和答案内容保持一致', async ({ page }, testInfo) => {
  const stem = `E2E 简答题映射 ${testInfo.project.name} ${Date.now()}`;
  const reference = '说明该功能的核心作用。';

  await loginAsAdmin(page, '/admin');
  await page.getByTestId('admin-question-type').selectOption('short_answer');
  await page.getByTestId('admin-stem').fill(stem);
  await page.getByTestId('admin-reference').fill(reference);
  await page.getByTestId('admin-rubric').fill('覆盖核心作用即可。');

  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === 'POST' && request.url().endsWith('/api/v1/admin/questions'),
  );
  await page.getByTestId('admin-save').click();
  const request = await requestPromise;
  expect(request.postDataJSON()).toMatchObject({
    question_type: 'short_answer',
    options: [],
    correct_answer: {
      type: 'short_answer',
      reference,
      rubric: '覆盖核心作用即可。',
    },
  });
});

test('选择题的 A-D 选项按显示顺序提交', async ({ page }, testInfo) => {
  const stem = `E2E A-D 选项 ${testInfo.project.name} ${Date.now()}`;

  await loginAsAdmin(page, '/admin');
  await page.getByTestId('admin-stem').fill(stem);
  await page.getByTestId('admin-option-A').fill('选项 A');
  await page.getByTestId('admin-option-B').fill('选项 B');
  await page.getByTestId('admin-option-C').fill('选项 C');
  await page.getByTestId('admin-option-D').fill('选项 D');
  await page.getByTestId('admin-correct-option').selectOption('D');

  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === 'POST' && request.url().endsWith('/api/v1/admin/questions'),
  );
  await page.getByTestId('admin-save').click();
  const request = await requestPromise;
  expect(request.postDataJSON()).toMatchObject({
    options: [
      { key: 'A', text: '选项 A' },
      { key: 'B', text: '选项 B' },
      { key: 'C', text: '选项 C' },
      { key: 'D', text: '选项 D' },
    ],
    correct_answer: { type: 'single_choice', option_key: 'D' },
  });
});

test('管理员可以为多选题添加可变数量选项并提交多个正确答案', async ({ page }, testInfo) => {
  const stem = `E2E 可变多选题 ${testInfo.project.name} ${Date.now()}`;

  await loginAsAdmin(page, '/admin');
  await page.getByTestId('admin-question-type').selectOption('multiple_choice');
  await page.getByTestId('admin-question-bank').selectOption({ label: '测试题库' });
  await page.getByTestId('admin-stem').fill(stem);
  await page.getByTestId('admin-option-A').fill('选项 A');
  await page.getByTestId('admin-option-B').fill('选项 B');
  await page.getByTestId('admin-option-C').fill('选项 C');
  await page.getByTestId('admin-option-D').fill('选项 D');

  await page.getByTestId('admin-add-option').click();
  await expect(page.getByTestId('admin-option-E')).toHaveCount(1);
  await page.getByTestId('admin-option-E').fill('选项 E');
  await page.getByTestId('admin-add-option').click();
  await expect(page.getByTestId('admin-option-F')).toHaveCount(1);
  await page.getByTestId('admin-remove-option-F').click();
  await expect(page.getByTestId('admin-option-F')).toHaveCount(0);

  await page.getByTestId('admin-correct-option-A').check();
  await page.getByTestId('admin-correct-option-C').check();
  await page.getByTestId('admin-correct-option-E').check();

  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === 'POST' && request.url().endsWith('/api/v1/admin/questions'),
  );
  await page.getByTestId('admin-save').click();
  const request = await requestPromise;
  expect(request.postDataJSON()).toMatchObject({
    question_type: 'multiple_choice',
    options: [
      { key: 'A', text: '选项 A' },
      { key: 'B', text: '选项 B' },
      { key: 'C', text: '选项 C' },
      { key: 'D', text: '选项 D' },
      { key: 'E', text: '选项 E' },
    ],
    correct_answer: { type: 'multiple_choice', option_keys: ['A', 'C', 'E'] },
  });
});

test('管理员可以创建、发布、下线题目', async ({ page }, testInfo) => {
  const stem = `E2E 管理题目 ${testInfo.project.name} ${Date.now()}`;

  await loginAsAdmin(page, '/admin');
  await expect(page.getByTestId('admin-page')).toBeVisible();

  await page.getByTestId('admin-question-type').selectOption('single_choice');
  await page.getByTestId('admin-question-bank').selectOption({ label: '测试题库' });
  await page.getByTestId('admin-stem').fill(stem);
  await page.getByTestId('admin-option-A').fill('错误选项');
  await page.getByTestId('admin-option-B').fill('正确选项');
  await page.getByTestId('admin-correct-option').selectOption('B');
  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === 'POST' && request.url().endsWith('/api/v1/admin/questions'),
  );
  await page.getByTestId('admin-save').click();
  const request = await requestPromise;
  expect(request.postDataJSON()).toMatchObject({
    question_bank_id: 2,
    question_type: 'single_choice',
    options: [
      { key: 'A', text: '错误选项' },
      { key: 'B', text: '正确选项' },
    ],
    correct_answer: { type: 'single_choice', option_key: 'B' },
  });

  const row = page.getByTestId('admin-question-row').filter({ hasText: stem });
  await expect(row).toContainText('草稿');

  await row.getByTestId('publish-question').click();
  await expect(row).toContainText('已发布');

  await page.goto('/');
  await expect(page.getByTestId('question-list')).toBeVisible();
  await page.getByTestId('question-bank-filter').selectOption({ label: '测试题库' });
  await expect(page.getByTestId('question-bank-filter')).toHaveValue('2');
  await openPracticeQuestion(page, stem);

  await page.goto('/admin');
  const publishedRow = page.getByTestId('admin-question-row').filter({ hasText: stem });
  await publishedRow.getByTestId('archive-question').click();
  await expect(publishedRow).toContainText('已下线');

  await page.goto('/');
  await expect(page.getByTestId('question-list')).toBeVisible();
  await page.getByTestId('question-bank-filter').selectOption({ label: '测试题库' });
  await expect(page.getByTestId('question-bank-filter')).toHaveValue('2');
  await expectPracticeQuestionAbsent(page, stem);
});

test('管理员可以创建题库并在题目编辑器中选择它', async ({ page }, testInfo) => {
  const bankName = `E2E 新题库 ${testInfo.project.name} ${Date.now()}`;

  await loginAsAdmin(page, '/admin');
  await expect(page.getByTestId('admin-question-bank-form')).toBeVisible();
  await page.getByTestId('admin-question-bank-name').fill(bankName);
  await page.getByTestId('admin-question-bank-description').fill('用于验证题库创建。');
  await page.getByTestId('admin-question-bank-save').click();

  await expect(page.getByTestId('admin-question-bank-list')).toContainText(bankName);
  await expect(page.getByTestId('admin-question-bank').locator('option').filter({ hasText: bankName })).toHaveCount(1);
  await page.getByTestId('admin-question-bank').selectOption({ label: bankName });
  await expect(page.getByTestId('admin-question-bank')).toHaveValue(/\d+/);
});

test('管理员可以使用关键字、题库、题型和状态筛选题目', async ({ page }, testInfo) => {
  const stem = `E2E 管理筛选 ${testInfo.project.name} ${Date.now()}`;

  await loginAsAdmin(page, '/admin');
  const typeFilter = page.getByTestId('admin-question-type-filter');
  const statusFilter = page.getByTestId('admin-question-status-filter');
  await expect(typeFilter).toHaveValue('');
  await expect(statusFilter).toHaveValue('');
  const bankFilter = page.getByTestId('admin-question-bank-filter');
  await bankFilter.selectOption({ label: '人工智能导论' });
  await expect(bankFilter).toHaveValue('1');
  const aiBankRequest = page.waitForRequest(
    (request) =>
      request.method() === 'GET' &&
      request.url().includes('/api/v1/admin/questions?') &&
      new URL(request.url()).searchParams.get('bank_id') === '1',
  );
  await page.getByTestId('admin-question-filter-submit').click();
  await aiBankRequest;

  await bankFilter.selectOption({ label: '测试题库' });
  await expect(bankFilter).toHaveValue('2');
  const testBankRequest = page.waitForRequest(
    (request) =>
      request.method() === 'GET' &&
      request.url().includes('/api/v1/admin/questions?') &&
      new URL(request.url()).searchParams.get('bank_id') === '2',
  );
  await page.getByTestId('admin-question-filter-submit').click();
  await testBankRequest;

  await page.getByTestId('admin-question-bank').selectOption({ label: '测试题库' });
  await page.getByTestId('admin-stem').fill(stem);
  await page.getByTestId('admin-option-A').fill('选项 A');
  await page.getByTestId('admin-option-B').fill('选项 B');
  await page.getByTestId('admin-correct-option').selectOption('B');
  await page.getByTestId('admin-save').click();
  await expect(page.getByTestId('admin-question-row').filter({ hasText: stem })).toContainText('草稿');

  await page.getByTestId('admin-question-keyword').fill(stem);
  const keywordRequest = page.waitForRequest(
    (request) => request.method() === 'GET' && request.url().includes('/api/v1/admin/questions?'),
  );
  await page.getByTestId('admin-question-filter-submit').click();
  const request = await keywordRequest;
  expect(new URL(request.url()).searchParams.get('keyword')).toBe(stem);
  await expect(page.getByTestId('admin-question-list').locator('[data-testid="admin-question-row"]')).toHaveCount(1);
  await expect(page.getByTestId('admin-question-list')).toContainText(stem);

  await page.getByTestId('admin-question-keyword').fill('');
  await page.getByTestId('admin-question-bank-filter').selectOption({ label: '测试题库' });
  await page.getByTestId('admin-question-type-filter').selectOption('single_choice');
  await expect(typeFilter).toHaveValue('single_choice');
  await page.getByTestId('admin-question-status-filter').selectOption('draft');
  await expect(statusFilter).toHaveValue('draft');
  const typeAndStatusRequest = page.waitForRequest(
    (request) =>
      request.method() === 'GET' &&
      request.url().includes('/api/v1/admin/questions?') &&
      new URL(request.url()).searchParams.get('bank_id') === '2' &&
      new URL(request.url()).searchParams.get('question_type') === 'single_choice' &&
      new URL(request.url()).searchParams.get('status') === 'draft',
  );
  await page.getByTestId('admin-question-filter-submit').click();
  await typeAndStatusRequest;
  await expect(page.getByTestId('admin-question-list')).toContainText(stem);
  await expect(page.getByTestId('admin-question-row').filter({ hasText: stem })).toContainText('选择题');

  await page.getByTestId('admin-question-filter-reset').click();
  await expect(page.getByTestId('admin-question-list')).toContainText(stem);
});

test('管理员可以在运行时批量导入题目并看到新增与跳过统计', async ({ page }, testInfo) => {
  const stem = `E2E 批量导入 ${testInfo.project.name} ${Date.now()}`;
  const payload = {
    items: [
      {
        question_bank_id: 2,
        question_type: 'single_choice',
        stem,
        blank_count: 0,
        options: [
          { key: 'A', text: '选项 A' },
          { key: 'B', text: '选项 B' },
        ],
        explanation: '批量导入解析。',
        correct_answer: { type: 'single_choice', option_key: 'B' },
      },
      {
        question_bank_id: 2,
        question_type: 'single_choice',
        stem: `  ${stem.toUpperCase()}  `,
        blank_count: 0,
        options: [
          { key: 'A', text: '覆盖选项 A' },
          { key: 'B', text: '覆盖选项 B' },
        ],
        explanation: '这条重复题不能覆盖已有题目。',
        correct_answer: { type: 'single_choice', option_key: 'A' },
      },
    ],
  };

  await loginAsAdmin(page, '/admin');
  await page.getByTestId('admin-question-import-json').fill(JSON.stringify(payload));
  await page.getByTestId('admin-question-import-submit').click();
  await expect(page.getByTestId('admin-question-import-result')).toHaveText(
    '导入完成：新增 1 道，跳过 1 道，错误 0 道。',
  );

  await page.getByTestId('admin-question-keyword').fill(stem);
  await page.getByTestId('admin-question-filter-submit').click();
  await expect(page.getByTestId('admin-question-row').filter({ hasText: stem })).toHaveCount(1);
  await expect(page.getByTestId('admin-question-row').filter({ hasText: stem })).toContainText('已发布');

  await page.goto('/');
  await expect(page.getByTestId('question-list')).toBeVisible();
  await page.getByTestId('question-bank-filter').selectOption({ label: '测试题库' });
  await expect(page.getByTestId('question-bank-filter')).toHaveValue('2');
  await openPracticeQuestion(page, stem);
});
