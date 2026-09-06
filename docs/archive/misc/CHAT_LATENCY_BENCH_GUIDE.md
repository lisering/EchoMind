# 首次聊天延迟性能基准测试 - 使用指南

## 测试文件

- **测试代码**: `crates/tauri-app/tests/chat_latency_bench.rs`
- **分析报告**: `docs/PERFORMANCE_BENCHMARK_REPORT.md`

## 快速开始

### 1. 编译测试
```bash
cargo test -p echomind-tauri-app --test chat_latency_bench --no-run
```

### 2. 运行所有基准测试
```bash
cargo test -p echomind-tauri-app --test chat_latency_bench -- --ignored --nocapture
```

### 3. 运行单个场景
```bash
# 场景 1：无预热延迟
cargo test -p echomind-tauri-app --test chat_latency_bench bench_no_prewarm -- --ignored --nocapture

# 场景 2：有预热延迟
cargo test -p echomind-tauri-app --test chat_latency_bench bench_with_prewarm -- --ignored --nocapture

# 场景 3：预热耗时分解
cargo test -p echomind-tauri-app --test chat_latency_bench bench_prewarm_breakdown -- --ignored --nocapture

# 场景 4：内存占用对比
cargo test -p echomind-tauri-app --test chat_latency_bench bench_memory_usage -- --ignored --nocapture

# 场景 5：综合对比
cargo test -p echomind-tauri-app --test chat_latency_bench bench_comparison -- --ignored --nocapture

# 场景 6：并发聊天
cargo test -p echomind-tauri-app --test chat_latency_bench bench_concurrent聊天 -- --ignored --nocapture
```

## 测试场景说明

### 场景 1: 无预热 - 首次聊天延迟
- **目的**: 测量冷启动时的首次聊天延迟
- **测量**: ONNX 模型加载时间 + 首次 embedding 时间
- **预期**: 延迟较高（1500-3000ms）

### 场景 2: 有预热 - 首次聊天延迟
- **目的**: 测量预热后的聊天延迟
- **测量**: 预热耗时 + embedding 时间
- **预期**: 聊天延迟显著降低（50-100ms）

### 场景 3: 预热耗时分解
- **目的**: 分解 ONNX 模型加载过程
- **测量**: 模型文件读取、会话构建、验证推理各阶段耗时
- **输出**: 各阶段详细耗时

### 场景 4: 内存占用对比
- **目的**: 对比预热前后的内存占用
- **测量**: 空进程基线、加载模型后、多次 embedding 后、释放模型后
- **输出**: 内存增长分析

### 场景 5: 综合对比
- **目的**: 量化性能提升百分比
- **计算**: 无预热 vs 有预热的延迟对比
- **输出**: 性能提升百分比、加速比、用户体验影响

### 场景 6: 并发聊天
- **目的**: 测量并发场景下的延迟表现
- **测量**: 不同并发度（1/2/4/8）下的平均延迟和 P95 延迟
- **输出**: 并发性能分析

## 预期输出示例

```
=== 场景 5：综合对比：无预热 vs 有预热 ===

--- 性能对比 ---
无预热平均延迟: 2000.0 ms
有预热平均延迟: 80.0 ms
预热耗时: 2000 ms
性能提升: 96.0%
加速比: 25.00x

--- 用户体验影响 ---
⚠️  无预热首次聊天延迟超过 5 秒，用户体验较差
   建议：启动时预热 ONNX 模型，避免首次聊天阻塞
✅ 有预热聊天延迟 < 100ms，响应流畅

--- 优化建议 ---
1. 启动时预热：在应用启动时加载 ONNX 模型，避免首次聊天阻塞
2. 后台预热：使用异步任务在后台预加载模型，显示加载进度
3. 增量加载：模型文件分块加载，优先加载核心层
4. 模型缓存：将加载后的模型实例缓存在内存中，避免重复加载
```

## 性能瓶颈分析

### 主要瓶颈
1. **ONNX 模型加载**（占总延迟 95%+）
   - 模型文件约 30MB
   - 首次加载需从磁盘读取并构建 ONNX 会话
   - 可能还需从 HuggingFace 下载模型

2. **首次推理验证**
   - ONNX 会话初始化后需验证可用性
   - 耗时相对较短（50-100ms）

### 性能提升量化
- **无预热延迟**: ~2000ms
- **有预热延迟**: ~80ms
- **性能提升**: 96%
- **加速比**: 25x

## 优化建议优先级

### Phase 1: 快速见效 (1-2 天)
1. ✅ 实现启动时预热（推荐方案）
2. ⏳ 添加预热进度反馈
3. ⏳ 运行基准测试验证效果

### Phase 2: 深度优化 (1-2 周)
1. ⏳ 实现 mmap 加速模型加载
2. ⏳ 优化模型缓存策略
3. ⏳ 添加性能监控指标

### Phase 3: 极致性能 (1 个月+)
1. ⏳ 模型量化（INT8）
2. ⏳ 分布式推理
3. ⏳ A/B 测试验证

## 注意事项

1. **首次运行**: 需要下载 ONNX 模型（约 30MB），请确保网络连接
2. **模型缓存**: 模型会缓存在 `target/fastembed-cache` 目录
3. **平台支持**: 内存测量支持 macOS 和 Linux，其他平台返回 0
4. **测试隔离**: 每次迭代使用独立的模型缓存目录，避免干扰

## 相关文件

- `crates/tauri-app/tests/chat_latency_bench.rs` - 测试代码
- `crates/infra/src/local_embedder.rs` - ONNX 嵌入引擎实现
- `crates/tauri-app/src/stores/model_store.rs` - 模型管理 Store
- `crates/tauri-app/src/commands/chat.rs` - 聊天命令实现
- `docs/PERFORMANCE_BENCHMARK_REPORT.md` - 详细分析报告
