import { defineConfig, devices } from '@playwright/test';

const port = Number(process.env.PLAYWRIGHT_PORT ?? 8000);
const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: './tests/e2e',
  timeout: 30_000,
  fullyParallel: false,
  reporter: process.env.CI ? 'line' : 'list',
  use: {
    baseURL,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: [
    {
      name: 'desktop-chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      name: 'mobile-chromium',
      use: { ...devices['iPhone 13'], browserName: 'chromium' },
    },
  ],
  webServer: {
    command:
      'env -u NO_COLOR cargo run --features server --bin xiaoluoquiz-server 2>&1',
    reuseExistingServer: false,
    timeout: 120_000,
    wait: {
      stdout: /xiaoluoquiz server listening/,
    },
    env: {
      ...process.env,
      APP_HOST: '127.0.0.1',
      APP_PORT: String(port),
      STATIC_DIR: 'dist',
      DATABASE_URL:
        process.env.DATABASE_URL ??
        'postgres://app:secret@127.0.0.1:5432/xiaoluoquiz',
      INITIAL_PASSWORD: process.env.INITIAL_PASSWORD ?? 'InitialPassword123!',
      INITIAL_ADMIN_USERNAME: process.env.INITIAL_ADMIN_USERNAME ?? 'demo-admin',
      INITIAL_ADMIN_DISPLAY_NAME: process.env.INITIAL_ADMIN_DISPLAY_NAME ?? '演示管理员',
    },
  },
});
