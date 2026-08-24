import { test, expect } from '@playwright/test';
import { enterApp, injectLocales, injectStub, uiUrl, openToolsDropdown, clickToolButton } from './helpers.mjs';

test.describe('对话分支/版本树（REQ-RAG-039）', () => {
  test.beforeEach(async ({ page }) => {
    await injectStub(page);
    await injectLocales(page);
    await page.goto(uiUrl);
    await enterApp(page);
  });

  test('TC-RAG-BRANCH-001: 分支树按钮存在且点击打开面板', async ({ page }) => {
    // S5 P1-1: branchTreeBtn 收纳到工具下拉菜单
    await openToolsDropdown(page);
    const btn = page.locator('#branchTreeBtn');
    await expect(btn).toBeVisible();

    // 点击按钮
    await btn.click();

    // 验证面板打开
    const panel = page.locator('#conversationTreePanel');
    await expect(panel).toBeVisible();

    // 验证面板标题
    const title = panel.locator('.conversation-tree-title');
    await expect(title).not.toBeEmpty();
  });

  test('TC-RAG-BRANCH-002: 空分支树显示空状态提示', async ({ page }) => {
    // 确保 mock 返回空树
    await page.evaluate(() => {
      window.__mock.state.conversationTree = {
        conversation_id: 'test-conv',
        nodes: [],
        root_ids: [],
        active_path: [],
      };
    });

    // 点击分支树按钮
    await clickToolButton(page, 'branchTreeBtn');

    // 验证空状态提示
    const emptyEl = page.locator('.conversation-tree-empty');
    await expect(emptyEl).toBeVisible();
    await expect(emptyEl).not.toBeEmpty();
  });

  test('TC-RAG-BRANCH-003: 分支树正确渲染节点结构', async ({ page }) => {
    // 设置 mock 分支树数据
    await page.evaluate(() => {
      window.__mock.state.conversationTree = {
        conversation_id: 'test-conv',
        nodes: [
          {
            node_id: 'turn-1#1',
            conversation_id: 'test-conv',
            parent_message_id: null,
            child_message_ids: ['turn-1#2'],
            active_child: 'turn-1#2',
            created_at: 1700000000,
            turn_group: 'turn-1',
            version: 1,
            preview: 'What is RAG?',
          },
          {
            node_id: 'turn-1#2',
            conversation_id: 'test-conv',
            parent_message_id: 'turn-1#1',
            child_message_ids: [],
            active_child: null,
            created_at: 1700000001,
            turn_group: 'turn-1',
            version: 2,
            preview: 'Explain RAG in detail',
          },
        ],
        root_ids: ['turn-1#1'],
        active_path: ['turn-1#1', 'turn-1#2'],
      };
    });

    // 点击分支树按钮
    await clickToolButton(page, 'branchTreeBtn');

    // 验证面板打开
    const panel = page.locator('#conversationTreePanel');
    await expect(panel).toBeVisible();

    // 验证节点渲染
    const nodes = panel.locator('.conversation-tree-node');
    await expect(nodes).toHaveCount(2);

    // 验证根节点版本号（用 > 直接子选择器避免匹配嵌套子节点）
    const rootVersion = nodes.first().locator('> .conversation-tree-node-content > .conversation-tree-node-version');
    await expect(rootVersion).toHaveText('v1');

    // 验证子节点版本号
    const childVersion = nodes.nth(1).locator('> .conversation-tree-node-content > .conversation-tree-node-version');
    await expect(childVersion).toHaveText('v2');

    // 验证预览内容
    const rootPreview = nodes.first().locator('> .conversation-tree-node-content > .conversation-tree-node-preview');
    await expect(rootPreview).toHaveText('What is RAG?');
  });

  test('TC-RAG-BRANCH-004: 活跃路径节点高亮显示', async ({ page }) => {
    await page.evaluate(() => {
      window.__mock.state.conversationTree = {
        conversation_id: 'test-conv',
        nodes: [
          {
            node_id: 'turn-1#1',
            conversation_id: 'test-conv',
            parent_message_id: null,
            child_message_ids: ['turn-1#2'],
            active_child: 'turn-1#2',
            created_at: 1700000000,
            turn_group: 'turn-1',
            version: 1,
            preview: 'Original question',
          },
          {
            node_id: 'turn-1#2',
            conversation_id: 'test-conv',
            parent_message_id: 'turn-1#1',
            child_message_ids: [],
            active_child: null,
            created_at: 1700000001,
            turn_group: 'turn-1',
            version: 2,
            preview: 'Edited question',
          },
        ],
        root_ids: ['turn-1#1'],
        active_path: ['turn-1#1', 'turn-1#2'],
      };
    });

    await clickToolButton(page, 'branchTreeBtn');

    const panel = page.locator('#conversationTreePanel');
    await expect(panel).toBeVisible();

    // 验证活跃节点有 .active 类
    const activeNodes = panel.locator('.conversation-tree-node.active');
    await expect(activeNodes).toHaveCount(2);

    // 验证活跃节点有 Active 徽章（用 > 直接子选择器避免匹配嵌套子节点）
    const badges = panel.locator('.conversation-tree-node.active > .conversation-tree-node-content > .conversation-tree-node-badge');
    await expect(badges).toHaveCount(2);
  });

  test('TC-RAG-BRANCH-005: branch_from_message IPC 正确创建新分支', async ({ page }) => {
    // 验证 branch_from_message IPC 调用
    const result = await page.evaluate(() => {
      return window.__TAURI__.core.invoke('branch_from_message', {
        conversationId: 'test-conv',
        messageId: 'msg-001',
        newContent: 'This is a branched question',
      });
    });

    // 验证返回结果
    expect(result).toBeTruthy();
    expect(result.new_version).toBe(2);
    expect(result.turn_group).toBe('turn-mock-branch');
  });
});
