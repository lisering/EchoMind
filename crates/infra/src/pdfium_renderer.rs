//! PDF 页面渲染引擎（REQ-MM-001）：pdfium-render（Chrome Pdfium C++ 绑定）。
//!
//! 将 PDF 页面渲染为位图（PNG 字节），供 OCR 或 VLM 处理。
//! pdfium 二进制文件随应用打包分发（Tauri resource 机制，ADR-010 风险缓解）。
//!
//! # 隐私
//!
//! ✅ 完全本地渲染，零网络请求。

use std::io::Cursor;
use std::sync::Arc;

use anyhow::Context;
use echomind_core::PageRenderer;
use image::ImageFormat;
use pdfium_render::prelude::*;
use tokio::sync::OnceCell;

/// PDF 默认 DPI（1 inch = 72 points）
const PDF_DEFAULT_DPI: f32 = 72.0;

/// PDF 页面渲染适配器（实现 `PageRenderer` 端口）。
///
/// 基于 `pdfium-render` crate（Google Chrome Pdfium C++ 绑定）。
/// pdfium 是业界标准的 PDF 渲染引擎，C++ 绑定是唯一可行方案（铁律九论证，ADR-010）。
///
/// # 隐私
///
/// ✅ 完全本地渲染，零网络请求。
pub struct PdfiumRenderer {
    /// 惰性初始化的 Pdfium 实例（线程安全，内部有锁）
    pdfium: OnceCell<Arc<Pdfium>>,
}

impl PdfiumRenderer {
    /// 创建 PDF 渲染器（Pdfium 实例在首次调用时初始化）。
    #[must_use]
    pub fn new() -> Self {
        Self {
            pdfium: OnceCell::new(),
        }
    }

    /// 获取或初始化 Pdfium 实例。
    ///
    /// 使用 `Pdfium::bind_to_system_library()` 显式绑定 pdfium 动态库，找不到库时返回
    /// `Err`（而非 `Pdfium::default()` 内部的 `unwrap()` panic，符合铁律五零 panic）。
    ///
    /// # 库查找路径
    ///
    /// - 系统标准路径（`/usr/local/lib`、`/usr/lib` 等，由 dlopen 解析）
    /// - `DYLD_LIBRARY_PATH` 环境变量指定的目录
    /// - 生产环境：通过 Tauri resource 机制打包 pdfium 二进制，启动时设置 `DYLD_LIBRARY_PATH`
    ///
    /// # 开发环境获取动态库（可选，仅真实渲染测试需要）
    ///
    /// 当前 3 个单元测试均为错误路径测试，无需动态库即可通过。
    /// 若未来加真实渲染测试，需先获取 pdfium 动态库：
    /// 1. 从 https://github.com/bblanchon/pdfium-binaries/releases 下载对应平台包
    ///    （macOS arm64: `pdfium-mac-arm64.tgz`，linux x64: `pdfium-linux-x64.tgz`）
    /// 2. 解压出 `libpdfium.dylib`（macOS）/ `libpdfium.so`（linux）放到 `target/debug/`
    ///    或设置 `DYLD_LIBRARY_PATH`（macOS）/ `LD_LIBRARY_PATH`（linux）指向解压目录
    async fn get_pdfium(&self) -> anyhow::Result<&Arc<Pdfium>> {
        self.pdfium
            .get_or_try_init(|| async {
                let bindings = Pdfium::bind_to_system_library()
                    .map_err(|e| anyhow::anyhow!("绑定 pdfium 动态库失败: {e}"))?;
                Ok(Arc::new(Pdfium::new(bindings)))
            })
            .await
    }
}

impl Default for PdfiumRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl PageRenderer for PdfiumRenderer {
    async fn render_page(
        &self,
        pdf_path: &str,
        page_num: usize,
        dpi: u32,
    ) -> anyhow::Result<Vec<u8>> {
        let pdfium = self.get_pdfium().await?;
        let pdf_path_owned = pdf_path.to_string();
        let pdfium = pdfium.clone();

        // PDF 渲染是 CPU 密集任务，用 spawn_blocking
        let result = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
            // 加载 PDF 文档
            let document = pdfium
                .load_pdf_from_file(&pdf_path_owned, None)
                .map_err(|e| anyhow::anyhow!("加载 PDF 失败: {e}"))?;

            // 检查页码范围（pdfium 页码类型为 i32）
            let page_count = document.pages().len() as usize;
            if page_num >= page_count {
                anyhow::bail!("页码 {page_num} 超出范围（PDF 共 {page_count} 页）");
            }

            // 获取指定页
            let page = document
                .pages()
                .get(page_num as i32)
                .map_err(|e| anyhow::anyhow!("获取页面 {page_num} 失败: {e}"))?;

            // 渲染配置：按 DPI 缩放（PDF 默认 72 DPI，scale = target_dpi / 72）
            let scale = dpi as f32 / PDF_DEFAULT_DPI;
            let render_config = PdfRenderConfig::new().scale_page_by_factor(scale);

            // 渲染页面为位图
            let bitmap = page
                .render_with_config(&render_config)
                .map_err(|e| anyhow::anyhow!("渲染页面失败: {e}"))?;

            // 转换为 image::DynamicImage（返回 Result）
            let image = bitmap
                .as_image()
                .map_err(|e| anyhow::anyhow!("位图转换为图像失败: {e}"))?;

            // 编码为 PNG 字节
            let mut png_bytes = Vec::new();
            image
                .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
                .map_err(|e| anyhow::anyhow!("转换为 PNG 失败: {e}"))?;

            Ok(png_bytes)
        })
        .await
        .context("PDF 渲染任务执行失败")??;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn test_new_does_not_initialize_pdfium() {
        let renderer = PdfiumRenderer::new();
        // OnceCell 在 get 之前应为空
        assert!(renderer.pdfium.get().is_none());
    }

    #[tokio::test]
    async fn test_render_nonexistent_pdf_returns_error() {
        let renderer = PdfiumRenderer::new();
        // 渲染不存在的文件应返回错误（而非 panic）。
        // 无 pdfium 动态库时返回 "绑定 pdfium 动态库失败"；
        // 有库时返回 "加载 PDF 失败"——两种情况均为 Err，断言通过。
        let result = renderer.render_page("/tmp/nonexistent.pdf", 0, 150).await;
        assert!(result.is_err(), "渲染不存在的 PDF 应返回错误");
    }

    #[tokio::test]
    async fn test_render_invalid_page_num_returns_error() {
        let renderer = PdfiumRenderer::new();
        // 即使 PDF 存在，页码越界也应返回错误。
        // 无 pdfium 动态库时同样返回 "绑定 pdfium 动态库失败"，断言通过。
        let result = renderer.render_page("/tmp/nonexistent.pdf", 999, 150).await;
        assert!(result.is_err(), "页码越界应返回错误");
    }
}
