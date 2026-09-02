import { expect, test } from '@playwright/test';

import {
  INITIAL_PASSWORD,
  loginAsAdmin,
  loginWithCredentials,
} from './helpers';

const QUESTION_STEM = 'Rust 的包管理工具是什么？';
const MULTIPLE_STEM = '以下哪些是 Rust 开发工具？';

function shanghaiDateTimeLocal(timestamp: number): string {
  return new Date(timestamp + 8 * 60 * 60 * 1_000).toISOString().slice(0, 16);
}

async function createPublishedPaper(page: Parameters<typeof loginAsAdmin>[0], title: string) {
  await loginAsAdmin(page, '/admin/papers');
  await expect(page.getByTestId('admin-papers-page')).toBeVisible();

  const question = page.getByTestId('admin-paper-question').filter({ hasText: QUESTION_STEM });
  await question.getByRole('checkbox').check();
  await page.getByTestId('selected-paper-question-list').getByRole('spinbutton').fill('2');
  await page.getByTestId('admin-paper-title').fill(title);
  await page.getByTestId('admin-paper-save').click();

  const row = page.getByTestId('admin-paper-row').filter({ hasText: title });
  await expect(row).toContainText('草稿');
  await row.getByTestId('publish-paper').click();
  await expect(row).toContainText('已发布');
  return row;
}
async function createPublishedPaperForQuestion(
  page: Parameters<typeof loginAsAdmin>[0],
  title: string,
  questionStem: string,
  score: string,
) {
  await loginAsAdmin(page, '/admin/papers');
  await expect(page.getByTestId('admin-papers-page')).toBeVisible();

  const question = page.getByTestId('admin-paper-question').filter({ hasText: questionStem });
  await question.getByRole('checkbox').check();
  await page.getByTestId('selected-paper-question-list').getByRole('spinbutton').fill(score);
  await page.getByTestId('admin-paper-title').fill(title);
  await page.getByTestId('admin-paper-save').click();

  const row = page.getByTestId('admin-paper-row').filter({ hasText: title });
  await expect(row).toContainText('草稿');
  await row.getByTestId('publish-paper').click();
  await expect(row).toContainText('已发布');
  return row;
}

async function createPublishedPaperForQuestions(
  page: Parameters<typeof loginAsAdmin>[0],
  title: string,
) {
  await loginAsAdmin(page, '/admin/papers');
  await expect(page.getByTestId('admin-papers-page')).toBeVisible();

  for (const stem of [QUESTION_STEM, MULTIPLE_STEM]) {
    await page
      .getByTestId('admin-paper-question')
      .filter({ hasText: stem })
      .getByRole('checkbox')
      .check();
  }
  const selected = page.getByTestId('selected-paper-question-list');
  await selected.getByTestId(/admin-paper-score-/).first().fill('2');
  await selected.getByTestId(/admin-paper-score-/).nth(1).fill('2');
  await page.getByTestId('admin-paper-title').fill(title);
  await page.getByTestId('admin-paper-save').click();

  const row = page.getByTestId('admin-paper-row').filter({ hasText: title });
  await expect(row).toContainText('草稿');
  await row.getByTestId('publish-paper').click();
  await expect(row).toContainText('已发布');
  return row;
}

test('试卷组装中的题型标签和题干与每个题目对应', async ({ page }) => {
  await loginAsAdmin(page, '/admin/papers');
  await expect(page.getByTestId('admin-papers-page')).toBeVisible();

  const singleChoice = page.getByTestId('admin-paper-question').filter({ hasText: QUESTION_STEM });
  const shortAnswer = page
    .getByTestId('admin-paper-question')
    .filter({ hasText: '请用一句话说明 SQLx 的作用。' });

  await expect(singleChoice.getByText('选择题', { exact: true })).toBeVisible();
  await expect(shortAnswer.getByText('简答题', { exact: true })).toBeVisible();

  await shortAnswer.getByRole('checkbox').check();
  const selected = page.getByTestId('selected-paper-question-list');
  await expect(selected).toContainText('请用一句话说明 SQLx 的作用。');
  await expect(selected).not.toContainText(QUESTION_STEM);
});

test('管理员可以组装、发布和下线试卷', async ({ page }, testInfo) => {
  const title = `E2E 试卷 ${testInfo.project.name} ${Date.now()}`;
  const row = await createPublishedPaper(page, title);

  await page.goto('/admin/papers');
  const publishedRow = page.getByTestId('admin-paper-row').filter({ hasText: title });
  await expect(publishedRow).toContainText('已发布');
  await publishedRow.getByTestId('archive-paper').click();
  await expect(publishedRow).toContainText('已下线');
  await expect(row).toHaveCount(1);
});

test('管理员设置开放和截止时间时提交带时区的时间', async ({ page }, testInfo) => {
  const title = `E2E 时间配置 ${testInfo.project.name} ${Date.now()}`;
  await loginAsAdmin(page, '/admin/papers');
  await expect(page.getByTestId('admin-papers-page')).toBeVisible();

  const requestPromise = page.waitForRequest(
    (request) =>
      request.method() === 'POST' && request.url().endsWith('/api/v1/admin/papers'),
  );
  const responsePromise = page.waitForResponse(
    (response) =>
      response.request().method() === 'POST' &&
      response.url().endsWith('/api/v1/admin/papers') &&
      response.status() === 201,
  );
  const question = page.getByTestId('admin-paper-question').filter({ hasText: QUESTION_STEM });
  await question.getByRole('checkbox').check();
  await page.getByTestId('admin-paper-title').fill(title);
  await page.getByTestId('admin-paper-open-at').fill('2030-01-02T03:04');
  await page.getByTestId('admin-paper-close-at').fill('2030-01-02T05:04');
  await page.getByTestId('admin-paper-save').click();

  const request = await requestPromise;
  const body = request.postDataJSON() as { open_at: string; close_at: string };
  expect(body.open_at).toBe('2030-01-02T03:04:00+08:00');
  expect(body.close_at).toBe('2030-01-02T05:04:00+08:00');

  const response = await responsePromise;
  const created = (await response.json()) as { open_at: string; close_at: string };
  expect(created.open_at).toBe('2030-01-02T03:04:00.000+08:00');
  expect(created.close_at).toBe('2030-01-02T05:04:00.000+08:00');

  const row = page.getByTestId('admin-paper-row').filter({ hasText: title });
  await expect(row).toContainText('草稿');
  await expect(row.getByTestId('admin-paper-open-time')).toContainText('2030-01-02 03:04:00');
  await expect(row.getByTestId('admin-paper-close-time')).toContainText('2030-01-02 05:04:00');
  await row.getByTestId('publish-paper').click();
  await expect(row).toContainText('已发布');
  await row.getByTestId('archive-paper').click();
  await expect(row).toContainText('已下线');
});

test('考试窗口在试卷列表和考试准备页按上海时间显示', async ({ page }, testInfo) => {
  const title = `E2E 上海时间窗口 ${testInfo.project.name} ${Date.now()}`;
  const openAt = shanghaiDateTimeLocal(Date.now() - 5 * 60 * 1_000);
  const closeAt = shanghaiDateTimeLocal(Date.now() + 2 * 60 * 60 * 1_000);
  await loginAsAdmin(page, '/admin/papers');
  await expect(page.getByTestId('admin-papers-page')).toBeVisible();

  const question = page.getByTestId('admin-paper-question').filter({ hasText: QUESTION_STEM });
  await question.getByRole('checkbox').check();
  await page.getByTestId('admin-paper-title').fill(title);
  await page.getByTestId('admin-paper-open-at').fill(openAt);
  await page.getByTestId('admin-paper-close-at').fill(closeAt);
  await page.getByTestId('admin-paper-save').click();

  const row = page.getByTestId('admin-paper-row').filter({ hasText: title });
  await expect(row).toContainText('草稿');
  await row.getByTestId('publish-paper').click();
  await expect(row).toContainText('已发布');

  await page.goto('/papers');
  await expect(page.getByTestId('paper-list')).toBeVisible();
  const paper = page.getByTestId('paper-card').filter({ hasText: title });
  await expect(paper.getByTestId('paper-open-time')).toContainText(`开放时间（上海时间）：${openAt.replace('T', ' ')}:00`);
  await expect(paper.getByTestId('paper-close-time')).toContainText(`截止时间（上海时间）：${closeAt.replace('T', ' ')}:00`);
  await paper.getByTestId('start-paper').click();
  await expect(page.getByTestId('paper-start-page')).toBeVisible();
  await expect(page.getByTestId('paper-start-open-time')).toContainText(`开放时间（上海时间）：${openAt.replace('T', ' ')}:00`);
  await expect(page.getByTestId('paper-start-close-time')).toContainText(`截止时间（上海时间）：${closeAt.replace('T', ' ')}:00`);

  await page.goto('/admin/papers');
  const publishedRow = page.getByTestId('admin-paper-row').filter({ hasText: title });
  await expect(publishedRow).toContainText('已发布');
  await publishedRow.getByTestId('archive-paper').click();
  await expect(publishedRow).toContainText('已下线');
});

test('学生可以填写考生信息、保存答案、刷新恢复并查看考试结果', async ({ page }, testInfo) => {
  const title = `E2E 正式考试 ${testInfo.project.name} ${Date.now()}`;
  await createPublishedPaper(page, title);

  const username = `e2e-exam-${testInfo.project.name}-${Date.now()}`;
  await page.goto('/admin/users');
  await expect(page.getByTestId('admin-users-page')).toBeVisible();
  await page.getByTestId('admin-user-username').fill(username);
  await page.getByTestId('admin-user-display-name').fill('E2E 考生');
  await page.getByTestId('admin-user-student-number').fill('20269998');
  await page.getByTestId('admin-user-save').click();
  await expect(page.getByTestId('admin-user-row').filter({ hasText: username })).toContainText('E2E 考生');

  await page.getByTestId('logout').click();
  await expect(page.getByTestId('login-page')).toBeVisible();
  await loginWithCredentials(page, username, INITIAL_PASSWORD);
  await expect(page.getByTestId('change-password-page')).toBeVisible();
  await page.getByTestId('change-password-new').fill('ExamStudentPassword123!');
  await page.getByTestId('change-password-confirm').fill('ExamStudentPassword123!');
  await page.getByTestId('change-password-submit').click();
  await expect(page.getByTestId('question-list')).toBeVisible();

  await page.goto('/papers');
  await expect(page.getByTestId('paper-list')).toBeVisible();
  const paper = page.getByTestId('paper-card').filter({ hasText: title });
  await paper.getByTestId('start-paper').click();
  await expect(page.getByTestId('paper-start-page')).toBeVisible();
  await page.getByTestId('candidate-student-number').fill('20269998');
  await page.getByTestId('start-exam').click();
  await expect(page.getByTestId('exam-page')).toBeVisible();
  await expect(page.getByTestId('exam-start-time')).toContainText('开始时间（上海时间）：');
  await expect(page.getByTestId('exam-deadline')).toContainText('截止时间（上海时间）：');

  const examQuestion = page.getByTestId('exam-question').filter({ hasText: QUESTION_STEM });
  await examQuestion.getByLabel(/B．Cargo/).check();
  await examQuestion.getByTestId('save-exam-answer').click();
  await expect(examQuestion.getByTestId('answer-save-status')).toContainText('已保存');

  await page.reload();
  await expect(page.getByTestId('exam-page')).toBeVisible();
  const reloadedQuestion = page.getByTestId('exam-question').filter({ hasText: QUESTION_STEM });
  await expect(reloadedQuestion.getByLabel(/B．Cargo/)).toBeChecked();

  await page.getByTestId('submit-exam').click();
  await expect(page.getByTestId('submit-exam-dialog')).toBeVisible();
  await page.getByTestId('confirm-submit-exam').click();
  await expect(page.getByTestId('exam-result-page')).toBeVisible();
  await expect(page.getByTestId('exam-result-page')).toContainText('2.00 / 2.00');
  await expect(page.getByTestId('exam-submitted-time')).toContainText('交卷时间（上海时间）：');
  await expect(page.getByTestId('exam-result-item').filter({ hasText: QUESTION_STEM })).toContainText('B');

  await page.reload();
  await expect(page.getByTestId('exam-result-page')).toBeVisible();
});

test('学生可以在正式考试中选择、保存和恢复多选题', async ({ page }, testInfo) => {
  const title = `E2E 多选正式考试 ${testInfo.project.name} ${Date.now()}`;
  await createPublishedPaperForQuestion(page, title, MULTIPLE_STEM, '2');

  const username = `e2e-multiple-exam-${testInfo.project.name}-${Date.now()}`;
  await page.goto('/admin/users');
  await expect(page.getByTestId('admin-users-page')).toBeVisible();
  await page.getByTestId('admin-user-username').fill(username);
  await page.getByTestId('admin-user-display-name').fill('E2E 多选考生');
  await page.getByTestId('admin-user-student-number').fill('20269997');
  await page.getByTestId('admin-user-save').click();
  await expect(page.getByTestId('admin-user-row').filter({ hasText: username })).toContainText('E2E 多选考生');

  await page.getByTestId('logout').click();
  await expect(page.getByTestId('login-page')).toBeVisible();
  await loginWithCredentials(page, username, INITIAL_PASSWORD);
  await expect(page.getByTestId('change-password-page')).toBeVisible();
  await page.getByTestId('change-password-new').fill('MultipleExamStudentPassword123!');
  await page.getByTestId('change-password-confirm').fill('MultipleExamStudentPassword123!');
  await page.getByTestId('change-password-submit').click();
  await expect(page.getByTestId('question-list')).toBeVisible();

  await page.goto('/papers');
  await expect(page.getByTestId('paper-list')).toBeVisible();
  const paper = page.getByTestId('paper-card').filter({ hasText: title });
  await paper.getByTestId('start-paper').click();
  await expect(page.getByTestId('paper-start-page')).toBeVisible();
  await page.getByTestId('candidate-student-number').fill('20269997');
  await page.getByTestId('start-exam').click();
  await expect(page.getByTestId('exam-page')).toBeVisible();

  const examQuestion = page.getByTestId('exam-question').filter({ hasText: MULTIPLE_STEM });
  await examQuestion.getByLabel(/E．Clippy/).check();
  await examQuestion.getByLabel(/A．Cargo/).check();
  await examQuestion.getByLabel(/C．rustc/).check();
  await expect(examQuestion.getByLabel(/B．npm/)).not.toBeChecked();
  await examQuestion.getByTestId('save-exam-answer').click();
  await expect(examQuestion.getByTestId('answer-save-status')).toContainText('已保存');

  await page.reload();
  await expect(page.getByTestId('exam-page')).toBeVisible();
  const reloadedQuestion = page.getByTestId('exam-question').filter({ hasText: MULTIPLE_STEM });
  await expect(reloadedQuestion.getByLabel(/E．Clippy/)).toBeChecked();
  await expect(reloadedQuestion.getByLabel(/A．Cargo/)).toBeChecked();
  await expect(reloadedQuestion.getByLabel(/C．rustc/)).toBeChecked();
  await expect(reloadedQuestion.getByLabel(/B．npm/)).not.toBeChecked();

  await page.getByTestId('submit-exam').click();
  await expect(page.getByTestId('submit-exam-dialog')).toBeVisible();
  await page.getByTestId('confirm-submit-exam').click();
  await expect(page.getByTestId('exam-result-page')).toBeVisible();
  await expect(page.getByTestId('exam-result-page')).toContainText('2.00 / 2.00');
  await expect(page.getByTestId('exam-result-item').filter({ hasText: MULTIPLE_STEM })).toContainText('E、A、C');
});

test('正式考试保存多选题遇到网络错误时显示友好提示', async ({ page }) => {
  const attemptId = 987654322;
  await page.route(`**/api/v1/attempts/${attemptId}`, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        id: attemptId,
        paper_id: 1,
        title: '多选题网络错误测试',
        status: 'in_progress',
        started_at: new Date().toISOString(),
        deadline_at: null,
        auto_submit: true,
        submitted_at: null,
        candidate_info: {},
        max_score: 2,
        total_score: null,
        unanswered_count: 1,
        questions: [
          {
            question_id: 76,
            revision_id: 76,
            position: 0,
            score: 2,
            question_type: 'multiple_choice',
            stem: MULTIPLE_STEM,
            blank_count: 0,
            options: [
              { key: 'A', text: 'Cargo' },
              { key: 'B', text: 'npm' },
              { key: 'C', text: 'rustc' },
              { key: 'D', text: 'pip' },
              { key: 'E', text: 'Clippy' },
            ],
            saved_answer: null,
            grading_status: 'pending',
            awarded_score: null,
          },
        ],
      }),
    });
  });
  await page.route(`**/api/v1/attempts/${attemptId}/answers`, async (route) => {
    await route.abort('failed');
  });

  await loginAsAdmin(page, `/exam/${attemptId}`);
  await expect(page.getByTestId('exam-page')).toBeVisible();
  const examQuestion = page.getByTestId('exam-question');
  await examQuestion.getByLabel(/E．Clippy/).check();
  await examQuestion.getByLabel(/A．Cargo/).check();
  await examQuestion.getByLabel(/C．rustc/).check();
  await examQuestion.getByTestId('save-exam-answer').click();
  await expect(examQuestion.getByRole('alert')).toContainText('网络连接失败');
  await expect(page.getByText('Load failed')).toHaveCount(0);
});

test('到达截止时间且启用自动交卷时会自动进入结果页', async ({ page }) => {
  const attemptId = 987654321;
  let submitCount = 0;
  const deadlineAt = new Date(Date.now() + 1_200).toISOString();
  await page.route(`**/api/v1/attempts/${attemptId}`, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        id: attemptId,
        paper_id: 1,
        title: '自动交卷测试',
        status: 'in_progress',
        started_at: new Date().toISOString(),
        deadline_at: deadlineAt,
        auto_submit: true,
        submitted_at: null,
        candidate_info: {},
        max_score: 2,
        total_score: null,
        unanswered_count: 1,
        questions: [
          {
            question_id: 1,
            revision_id: 1,
            position: 0,
            score: 2,
            question_type: 'single_choice',
            stem: QUESTION_STEM,
            blank_count: 0,
            options: [
              { key: 'A', text: 'npm' },
              { key: 'B', text: 'Cargo' },
            ],
            saved_answer: null,
            grading_status: 'pending',
            awarded_score: null,
          },
        ],
      }),
    });
  });
  await page.route(`**/api/v1/attempts/${attemptId}/result`, async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        attempt_id: attemptId,
        paper_id: 1,
        title: '自动交卷测试',
        status: 'graded',
        submitted_at: new Date().toISOString(),
        max_score: 2,
        total_score: 0,
        items: [
          {
            question_id: 1,
            position: 0,
            stem: QUESTION_STEM,
            question_type: 'single_choice',
            max_score: 2,
            answer: null,
            awarded_score: 0,
            answered: false,
            status: null,
            grading_status: 'graded',
            correct_answer: { type: 'single_choice', option_key: 'B' },
            explanation: null,
            feedback: null,
          },
        ],
      }),
    });
  });
  await page.route(`**/api/v1/attempts/${attemptId}/submit`, async (route) => {
    submitCount += 1;
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({
        attempt_id: attemptId,
        paper_id: 1,
        title: '自动交卷测试',
        status: 'graded',
        submitted_at: new Date().toISOString(),
        max_score: 2,
        total_score: 0,
        items: [
          {
            question_id: 1,
            position: 0,
            stem: QUESTION_STEM,
            question_type: 'single_choice',
            max_score: 2,
            answer: null,
            awarded_score: 0,
            answered: false,
            status: null,
            grading_status: 'graded',
            correct_answer: { type: 'single_choice', option_key: 'B' },
            explanation: null,
            feedback: null,
          },
        ],
      }),
    });
  });

  await loginAsAdmin(page, `/exam/${attemptId}`);
  await expect(page.getByTestId('exam-page')).toBeVisible();
  await expect(page.getByTestId('exam-start-time')).toHaveText(/开始时间（上海时间）：\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
  await expect(page.getByTestId('exam-deadline')).toHaveText(/截止时间（上海时间）：\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
  await expect(page.getByTestId('exam-result-page')).toBeVisible({ timeout: 8_000 });
  await expect(page.getByTestId('exam-submitted-time')).toHaveText(/交卷时间（上海时间）：\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}/);
  await expect(page.getByTestId('exam-total-score')).toContainText('0.00 / 2.00');
  expect(submitCount).toBe(1);
});

test('试卷列表的空状态和加载失败状态可见', async ({ page }) => {
  await loginAsAdmin(page, '/papers');

  await page.route('**/api/v1/papers', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ items: [] }),
    });
  });
  await page.reload();
  await expect(page.getByTestId('paper-empty-state')).toBeVisible();

  await page.unroute('**/api/v1/papers');
  await page.route('**/api/v1/papers', async (route) => {
    await route.abort('failed');
  });
  await page.reload();
  await expect(page.getByTestId('paper-error-state')).toBeVisible();
});

test('试卷页面在移动端没有横向溢出', async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await loginAsAdmin(page, '/admin/papers');
  await expect(page.getByTestId('admin-papers-page')).toBeVisible();
  await page.goto('/papers');
  await expect(page.getByTestId('paper-list')).toBeVisible();
  const hasHorizontalOverflow = await page.evaluate(
    () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
  );
  expect(hasHorizontalOverflow).toBe(false);

  const list = page.getByTestId('published-paper-list');
  const [listRect, cardRects] = await Promise.all([
    list.evaluate((node) => {
      const rect = node.getBoundingClientRect();
      return { right: rect.right, width: rect.width };
    }),
    page.getByTestId('paper-card').evaluateAll((nodes) =>
      nodes.map((node) => {
        const rect = node.getBoundingClientRect();
        return { right: rect.right, width: rect.width };
      }),
    ),
  ]);
  for (const cardRect of cardRects) {
    expect(cardRect.width).toBeLessThanOrEqual(listRect.width + 1);
    expect(cardRect.right).toBeLessThanOrEqual(listRect.right + 1);
  }
});

test('正式考试支持顺序导航、题号跳转并在交卷前保存最新答案', async ({ page }, testInfo) => {
  const title = `E2E 考试导航 ${testInfo.project.name} ${Date.now()}`;
  await createPublishedPaperForQuestions(page, title);

  const username = `e2e-navigation-${testInfo.project.name}-${Date.now()}`;
  await page.goto('/admin/users');
  await expect(page.getByTestId('admin-users-page')).toBeVisible();
  await page.getByTestId('admin-user-username').fill(username);
  await page.getByTestId('admin-user-display-name').fill('导航考生');
  await page.getByTestId('admin-user-student-number').fill('20269996');
  await page.getByTestId('admin-user-save').click();
  await expect(page.getByTestId('admin-user-row').filter({ hasText: username })).toContainText('导航考生');

  await page.getByTestId('logout').click();
  await expect(page.getByTestId('login-page')).toBeVisible();
  await loginWithCredentials(page, username, INITIAL_PASSWORD);
  await expect(page.getByTestId('change-password-page')).toBeVisible();
  await page.getByTestId('change-password-new').fill('NavigationStudentPassword123!');
  await page.getByTestId('change-password-confirm').fill('NavigationStudentPassword123!');
  await page.getByTestId('change-password-submit').click();
  await expect(page.getByTestId('question-list')).toBeVisible();

  await page.goto('/papers');
  await page.getByTestId('paper-card').filter({ hasText: title }).getByTestId('start-paper').click();
  await page.getByTestId('candidate-student-number').fill('20269996');
  await page.getByTestId('start-exam').click();
  await expect(page.getByTestId('exam-page')).toBeVisible();
  const hasHorizontalOverflow = await page.evaluate(
    () => document.documentElement.scrollWidth > document.documentElement.clientWidth,
  );
  expect(hasHorizontalOverflow).toBe(false);
  const navigationToggle = page.getByTestId('exam-question-numbers-toggle');
  const navigationPanel = page.getByTestId('exam-question-navigation-panel');
  await expect(navigationToggle).toHaveAttribute('aria-expanded', 'false');
  await expect(navigationPanel).toHaveCount(0);
  await navigationToggle.click();
  await expect(navigationToggle).toHaveAttribute('aria-expanded', 'true');
  await expect(navigationPanel).toBeVisible();
  await expect(navigationPanel.getByRole('button')).toHaveCount(2);
  await expect(page.getByTestId('exam-question')).toContainText(QUESTION_STEM);

  await page.getByTestId('exam-question').getByLabel(/B．Cargo/).check();
  await expect(page.getByTestId('exam-question-number-0')).toHaveAttribute('data-answered', 'true');
  await page.getByTestId('exam-next').click();
  await expect(page.getByTestId('exam-question')).toContainText(MULTIPLE_STEM);
  await page.getByTestId('exam-question').getByLabel(/E．Clippy/).check();
  await page.getByTestId('exam-question').getByLabel(/A．Cargo/).check();
  await page.getByTestId('exam-question').getByLabel(/C．rustc/).check();

  await page.getByTestId('exam-question-number-0').click();
  await expect(page.getByTestId('exam-question')).toContainText(QUESTION_STEM);
  await expect(page.getByTestId('exam-question').getByLabel(/B．Cargo/)).toBeChecked();
  await page.getByTestId('exam-question-number-1').click();
  await expect(page.getByTestId('exam-question')).toContainText(MULTIPLE_STEM);
  await expect(page.getByTestId('exam-question').getByLabel(/E．Clippy/)).toBeChecked();

  await page.getByTestId('submit-exam').click();
  await page.getByTestId('confirm-submit-exam').click();
  await expect(page.getByTestId('exam-result-page')).toBeVisible();
  await expect(page.getByTestId('exam-total-score')).toContainText('4.00 / 4.00');
  await expect(page.getByText('Load failed')).toHaveCount(0);
});

test('管理员可以查看已交卷记录并批改简答题', async ({ page }, testInfo) => {
  const title = `E2E 管理阅卷 ${testInfo.project.name} ${Date.now()}`;
  await createPublishedPaperForQuestion(page, title, '请用一句话说明 SQLx 的作用。', '2');

  const username = `e2e-grading-${testInfo.project.name}-${Date.now()}`;
  await page.goto('/admin/users');
  await expect(page.getByTestId('admin-users-page')).toBeVisible();
  await page.getByTestId('admin-user-username').fill(username);
  await page.getByTestId('admin-user-display-name').fill('待批改考生');
  await page.getByTestId('admin-user-student-number').fill('20269995');
  await page.getByTestId('admin-user-save').click();
  await expect(page.getByTestId('admin-user-row').filter({ hasText: username })).toContainText('待批改考生');

  await page.getByTestId('logout').click();
  await expect(page.getByTestId('login-page')).toBeVisible();
  await loginWithCredentials(page, username, INITIAL_PASSWORD);
  await expect(page.getByTestId('change-password-page')).toBeVisible();
  await page.getByTestId('change-password-new').fill('GradingStudentPassword123!');
  await page.getByTestId('change-password-confirm').fill('GradingStudentPassword123!');
  await page.getByTestId('change-password-submit').click();
  await expect(page.getByTestId('question-list')).toBeVisible();

  await page.goto('/papers');
  await page.getByTestId('paper-card').filter({ hasText: title }).getByTestId('start-paper').click();
  await page.getByTestId('candidate-student-number').fill('20269995');
  await page.getByTestId('start-exam').click();
  await expect(page.getByTestId('exam-page')).toBeVisible();
  await page.getByTestId('exam-question').locator('textarea').fill('SQLx 为 Rust 提供异步数据库访问能力。');
  await page.getByTestId('save-exam-answer').click();
  await expect(page.getByTestId('answer-save-status')).toContainText('已保存');
  await page.getByTestId('submit-exam').click();
  await page.getByTestId('confirm-submit-exam').click();
  await expect(page.getByTestId('exam-result-page')).toBeVisible();
  await expect(page.getByTestId('exam-total-score')).toContainText('待批改');

  await page.getByTestId('logout').click();
  await expect(page.getByTestId('login-page')).toBeVisible();
  await loginAsAdmin(page, '/admin/attempts');
  await expect(page.getByTestId('admin-attempt-list')).toContainText(title);
  const attemptRow = page.getByTestId('admin-attempt-row').filter({ hasText: title });
  await attemptRow.getByTestId('view-admin-attempt').click();
  await expect(page.getByTestId('admin-attempt-detail-page')).toBeVisible();
  await expect(page.getByTestId('admin-attempt-detail-page')).toContainText('SQLx 为 Rust 提供异步数据库访问能力。');

  await page.getByTestId('admin-grade-score').fill('1.5');
  await page.getByTestId('admin-grade-feedback').fill('答案准确，表述清楚。');
  await page.getByTestId('admin-grade-save').click();
  await expect(page.getByTestId('admin-attempt-status')).toContainText('已评分');
  await expect(page.getByTestId('admin-attempt-total-score')).toContainText('1.50 / 2.00');
  await expect(page.getByTestId('admin-attempt-detail-page')).toContainText('答案准确，表述清楚。');
});
