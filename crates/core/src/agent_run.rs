//! AgentEngine Sans-IO 状态机（B-03 借鉴 Rig `AgentRun`）。
//!
//! ## 核心设计
//!
//! 将 Agent ReAct loop 的**决策逻辑**与 **IO 执行**完全分离：
//!
//! - `AgentRunState` — 纯决策状态枚举（`CallModel` / `CallTools` / `Done`），
//!   不含任何 IO 操作，可独立单元测试
//! - `AgentRunStep` — 状态机的下一步指令（IO 执行层据此执行 LLM 调用或工具执行）
//! - `AgentRunMachine` — 状态机驱动器，接收 IO 结果后推进状态
//!
//! ## 决策与 IO 分离的好处
//!
//! 1. **可单元测试**：决策逻辑无需 mock LLM/Retriever（只测状态转换正确性）
//! 2. **可序列化**：状态可持久化，未来支持崩溃恢复
//! 3. **可复用**：Coordinator / SpeculativeRAG 可复用同一状态机
//!
//! ## 状态转换图
//!
//! ```text
//! Init → CallModel → (解析响应)
//!   ├── FinalAnswer → Done
//!   ├── ThoughtAndAction → CallTools → (执行工具) → CallModel
//!   ├── ThoughtAndMultiActions → CallTools → (执行工具) → CallModel
//!   └── Unparseable → Done (降级为标准 RAG)
//! ```

use serde::{Deserialize, Serialize};

use crate::agent::ReactParse;
use crate::finish_reason::FinishReason;

/// Agent 运行状态（纯决策，不含 IO）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentRunState {
    /// 初始状态：准备调用 LLM
    Init,
    /// 调用 LLM 模型：需要 IO 层执行 `chat_stream` 并回填结果
    CallModel {
        /// 当前迭代轮次（从 1 开始）
        turn: usize,
    },
    /// 执行工具：需要 IO 层执行工具并回填结果
    CallTools {
        /// 当前迭代轮次
        turn: usize,
        /// 待执行的工具调用列表
        actions: Vec<(String, String)>,
    },
    /// 完成：Agent 运行结束
    Done {
        /// 结束原因
        reason: DoneReason,
    },
}

/// Agent 运行结束原因。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DoneReason {
    /// LLM 输出了最终答案
    FinalAnswer,
    /// 达到最大迭代次数
    MaxIterations,
    /// LLM 响应无法解析，降级为标准 RAG
    DegradedToRag,
    /// 知识库未命中，无上下文
    NoContext,
}

/// 状态机的下一步指令（IO 执行层据此执行操作）。
#[derive(Debug, Clone, PartialEq)]
pub enum AgentRunStep {
    /// 需要 IO 层调用 LLM：执行 `chat_stream` 后将结果通过 `model_response()` 回填
    CallModel {
        /// 当前迭代轮次
        turn: usize,
    },
    /// 需要 IO 层执行工具：执行后通过 `tool_results()` 回填结果
    CallTools {
        /// 当前迭代轮次
        turn: usize,
        /// 待执行的工具调用列表 `[(tool_name, tool_input)]`
        actions: Vec<(String, String)>,
    },
    /// 状态机已完成，IO 层可清理并返回结果
    Done {
        /// 结束原因
        reason: DoneReason,
    },
}

/// Sans-IO Agent 状态机驱动器。
///
/// 纯决策逻辑：接收解析后的 LLM 响应 → 决定下一步（CallModel / CallTools / Done）。
/// 不执行任何 IO 操作（不调用 LLM、不执行工具）。
///
/// # 使用方式
///
/// ```text
/// let mut machine = AgentRunMachine::new(max_iterations);
/// loop {
///     match machine.next_step() {
///         AgentRunStep::CallModel { turn } => {
///             // IO 层：调用 LLM，解析响应
///             let response = llm.chat_stream(...).collect();
///             let parsed = parse_react_response(&response);
///             machine.model_response(parsed, finish_reason);
///         }
///         AgentRunStep::CallTools { turn, actions } => {
///             // IO 层：执行工具
///             let results = execute_tools(actions);
///             machine.tool_results(results);
///         }
///         AgentRunStep::Done { reason } => break,
///     }
/// }
/// ```
pub struct AgentRunMachine {
    /// 当前状态
    state: AgentRunState,
    /// 最大迭代次数
    max_iterations: usize,
    /// 已完成的轮次数
    turns_completed: usize,
}

impl AgentRunMachine {
    /// 创建新状态机。
    ///
    /// # 参数
    /// - `max_iterations`: 最大迭代次数（超过后强制 Done）
    pub fn new(max_iterations: usize) -> Self {
        Self {
            state: AgentRunState::Init,
            max_iterations,
            turns_completed: 0,
        }
    }

    /// 获取当前状态。
    pub fn state(&self) -> &AgentRunState {
        &self.state
    }

    /// 已完成的轮次数。
    pub fn turns_completed(&self) -> usize {
        self.turns_completed
    }

    /// 推进到下一步（纯决策，不执行 IO）。
    ///
    /// 根据当前状态返回下一步指令并推进内部状态：
    /// - `Init` → 转为 `CallModel { turn: 1 }`，返回 `CallModel { turn: 1 }`
    /// - `CallModel` → 返回 `CallModel { turn }`（等待 `model_response()` 回填）
    /// - `CallTools` → 返回 `CallTools { turn, actions }`（等待 `tool_results()` 回填）
    /// - `Done` → 返回 `Done { reason }`
    pub fn next_step(&mut self) -> AgentRunStep {
        match &self.state {
            AgentRunState::Init => {
                self.state = AgentRunState::CallModel { turn: 1 };
                AgentRunStep::CallModel { turn: 1 }
            }
            AgentRunState::CallModel { turn } => AgentRunStep::CallModel { turn: *turn },
            AgentRunState::CallTools { turn, actions } => AgentRunStep::CallTools {
                turn: *turn,
                actions: actions.clone(),
            },
            AgentRunState::Done { reason } => AgentRunStep::Done {
                reason: reason.clone(),
            },
        }
    }

    /// 回填 LLM 响应结果（IO 层调用 LLM 后调用此方法）。
    ///
    /// 根据 ReAct 解析结果决定状态转换：
    /// - `FinalAnswer` → `Done(FinalAnswer)`
    /// - `ThoughtAndAction` → `CallTools`（单个工具）
    /// - `ThoughtAndMultiActions` → `CallTools`（多个工具）
    /// - `Unparseable` → `Done(DegradedToRag)`
    ///
    /// 同时检查最大迭代次数：若 `turn >= max_iterations`，强制 `Done(MaxIterations)`。
    ///
    /// # 参数
    /// - `parsed`: ReAct 解析结果
    /// - `_finish_reason`: LLM 的停止原因（当前仅用于日志，未来可驱动截断重试）
    pub fn model_response(&mut self, parsed: ReactParse, _finish_reason: FinishReason) {
        let turn = match &self.state {
            AgentRunState::CallModel { turn } => *turn,
            _ => return, // 状态不匹配，忽略
        };

        self.turns_completed = turn;

        // 检查最大迭代次数
        if turn >= self.max_iterations {
            // 即使有 Action，也强制结束
            match parsed {
                ReactParse::FinalAnswer(_) => {
                    self.state = AgentRunState::Done {
                        reason: DoneReason::FinalAnswer,
                    };
                }
                _ => {
                    self.state = AgentRunState::Done {
                        reason: DoneReason::MaxIterations,
                    };
                }
            }
            return;
        }

        match parsed {
            ReactParse::FinalAnswer(_) => {
                self.state = AgentRunState::Done {
                    reason: DoneReason::FinalAnswer,
                };
            }
            ReactParse::ThoughtAndAction { tool, input, .. } => {
                self.state = AgentRunState::CallTools {
                    turn,
                    actions: vec![(tool, input)],
                };
            }
            ReactParse::ThoughtAndMultiActions { actions, .. } => {
                self.state = AgentRunState::CallTools { turn, actions };
            }
            ReactParse::Unparseable => {
                self.state = AgentRunState::Done {
                    reason: DoneReason::DegradedToRag,
                };
            }
        }
    }

    /// 回填工具执行结果（IO 层执行工具后调用此方法）。
    ///
    /// 将状态从 `CallTools` 推进到下一轮 `CallModel`。
    ///
    /// # 参数
    /// - `_results`: 工具执行结果列表（IO 层已处理，状态机不需要内容）
    pub fn tool_results(&mut self, _results: Vec<String>) {
        let turn = match &self.state {
            AgentRunState::CallTools { turn, .. } => *turn,
            _ => return, // 状态不匹配，忽略
        };

        // 推进到下一轮
        self.state = AgentRunState::CallModel { turn: turn + 1 };
    }

    /// 是否已完成。
    pub fn is_done(&self) -> bool {
        matches!(self.state, AgentRunState::Done { .. })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::agent::ReactParse;
    use crate::finish_reason::FinishReason;

    // ─── 初始化 ───

    #[test]
    fn tc_run_001_init_next_step_is_call_model() {
        let mut machine = AgentRunMachine::new(5);
        assert_eq!(machine.next_step(), AgentRunStep::CallModel { turn: 1 });
        assert!(!machine.is_done());
    }

    #[test]
    fn tc_run_002_init_state() {
        let machine = AgentRunMachine::new(5);
        assert_eq!(machine.state(), &AgentRunState::Init);
        assert_eq!(machine.turns_completed(), 0);
    }

    // ─── FinalAnswer → Done ───

    #[test]
    fn tc_run_003_final_answer_transitions_to_done() {
        let mut machine = AgentRunMachine::new(5);
        // Init → CallModel
        machine.next_step();

        // 回填 FinalAnswer
        machine.model_response(
            ReactParse::FinalAnswer("The answer is 42".to_string()),
            FinishReason::Stop,
        );

        assert!(machine.is_done());
        assert_eq!(
            machine.state(),
            &AgentRunState::Done {
                reason: DoneReason::FinalAnswer
            }
        );
    }

    // ─── ThoughtAndAction → CallTools → CallModel ───

    #[test]
    fn tc_run_004_thought_and_action_transitions_to_call_tools() {
        let mut machine = AgentRunMachine::new(5);
        machine.next_step(); // Init → CallModel(1)

        machine.model_response(
            ReactParse::ThoughtAndAction {
                thought: "I need to search".to_string(),
                tool: "search_kb".to_string(),
                input: "rust async".to_string(),
            },
            FinishReason::Stop,
        );

        assert_eq!(
            machine.next_step(),
            AgentRunStep::CallTools {
                turn: 1,
                actions: vec![("search_kb".to_string(), "rust async".to_string())],
            }
        );
    }

    #[test]
    fn tc_run_005_tool_results_transitions_to_next_call_model() {
        let mut machine = AgentRunMachine::new(5);
        machine.next_step(); // Init → CallModel(1)

        // CallModel(1) → CallTools(1)
        machine.model_response(
            ReactParse::ThoughtAndAction {
                thought: "search".to_string(),
                tool: "search_kb".to_string(),
                input: "query".to_string(),
            },
            FinishReason::Stop,
        );

        // 回填工具结果 → CallModel(2)
        machine.tool_results(vec!["found results".to_string()]);

        assert_eq!(machine.next_step(), AgentRunStep::CallModel { turn: 2 });
        assert_eq!(machine.turns_completed(), 1);
    }

    // ─── ThoughtAndMultiActions → CallTools ───

    #[test]
    fn tc_run_006_multi_actions_transitions_to_call_tools() {
        let mut machine = AgentRunMachine::new(5);
        machine.next_step(); // Init → CallModel(1)

        machine.model_response(
            ReactParse::ThoughtAndMultiActions {
                thought: "search multiple".to_string(),
                actions: vec![
                    ("search_kb".to_string(), "query1".to_string()),
                    ("search_kb".to_string(), "query2".to_string()),
                ],
            },
            FinishReason::Stop,
        );

        match machine.next_step() {
            AgentRunStep::CallTools { turn, actions } => {
                assert_eq!(turn, 1);
                assert_eq!(actions.len(), 2);
            }
            _ => panic!("Expected CallTools"),
        }
    }

    // ─── Unparseable → Done(DegradedToRag) ───

    #[test]
    fn tc_run_007_unparseable_transitions_to_done_degraded() {
        let mut machine = AgentRunMachine::new(5);
        machine.next_step(); // Init → CallModel(1)

        machine.model_response(ReactParse::Unparseable, FinishReason::Stop);

        assert!(machine.is_done());
        assert_eq!(
            machine.state(),
            &AgentRunState::Done {
                reason: DoneReason::DegradedToRag
            }
        );
    }

    // ─── Max iterations ───

    #[test]
    fn tc_run_008_max_iterations_forces_done() {
        let mut machine = AgentRunMachine::new(3);
        machine.next_step(); // Init → CallModel(1)

        // 第 1 轮
        machine.model_response(
            ReactParse::ThoughtAndAction {
                thought: "t1".to_string(),
                tool: "search_kb".to_string(),
                input: "q1".to_string(),
            },
            FinishReason::Stop,
        );
        machine.tool_results(vec!["r1".to_string()]);

        // 第 2 轮
        machine.model_response(
            ReactParse::ThoughtAndAction {
                thought: "t2".to_string(),
                tool: "search_kb".to_string(),
                input: "q2".to_string(),
            },
            FinishReason::Stop,
        );
        machine.tool_results(vec!["r2".to_string()]);

        // 第 3 轮（turn == max_iterations）
        machine.model_response(
            ReactParse::ThoughtAndAction {
                thought: "t3".to_string(),
                tool: "search_kb".to_string(),
                input: "q3".to_string(),
            },
            FinishReason::Stop,
        );

        // 应强制 Done(MaxIterations)
        assert!(machine.is_done());
        assert_eq!(
            machine.state(),
            &AgentRunState::Done {
                reason: DoneReason::MaxIterations
            }
        );
    }

    #[test]
    fn tc_run_009_max_iterations_with_final_answer_still_done_final() {
        let mut machine = AgentRunMachine::new(1);
        machine.next_step(); // Init → CallModel(1)

        // 第 1 轮直接 FinalAnswer（turn == max_iterations）
        machine.model_response(
            ReactParse::FinalAnswer("answer".to_string()),
            FinishReason::Stop,
        );

        assert!(machine.is_done());
        assert_eq!(
            machine.state(),
            &AgentRunState::Done {
                reason: DoneReason::FinalAnswer
            }
        );
    }

    // ─── 完整循环测试 ───

    #[test]
    fn tc_run_010_full_loop_two_rounds_then_answer() {
        let mut machine = AgentRunMachine::new(5);

        // 第 1 轮：搜索
        machine.next_step(); // Init → CallModel(1)
        machine.model_response(
            ReactParse::ThoughtAndAction {
                thought: "need to search".to_string(),
                tool: "search_kb".to_string(),
                input: "rust".to_string(),
            },
            FinishReason::Stop,
        );
        assert_eq!(
            machine.next_step(),
            AgentRunStep::CallTools {
                turn: 1,
                actions: vec![("search_kb".to_string(), "rust".to_string())],
            }
        );
        machine.tool_results(vec!["found rust docs".to_string()]);

        // 第 2 轮：最终答案
        assert_eq!(machine.next_step(), AgentRunStep::CallModel { turn: 2 });
        machine.model_response(
            ReactParse::FinalAnswer("Rust is a systems language".to_string()),
            FinishReason::Stop,
        );

        assert!(machine.is_done());
        assert_eq!(machine.turns_completed(), 2);
    }

    // ─── 状态不匹配时忽略 ───

    #[test]
    fn tc_run_011_model_response_in_wrong_state_ignored() {
        let mut machine = AgentRunMachine::new(5);
        // 在 Init 状态下调用 model_response（未先 next_step）
        machine.model_response(
            ReactParse::FinalAnswer("answer".to_string()),
            FinishReason::Stop,
        );
        // 状态不应改变（仍在 Init）
        assert_eq!(machine.state(), &AgentRunState::Init);
    }

    #[test]
    fn tc_run_012_tool_results_in_wrong_state_ignored() {
        let mut machine = AgentRunMachine::new(5);
        // 在 Init 状态下调用 tool_results
        machine.tool_results(vec!["result".to_string()]);
        // 状态不应改变
        assert_eq!(machine.state(), &AgentRunState::Init);
    }

    // ─── 序列化 ───

    #[test]
    fn tc_run_013_state_serializable() {
        let state = AgentRunState::CallTools {
            turn: 3,
            actions: vec![("search_kb".to_string(), "query".to_string())],
        };
        let json = serde_json::to_string(&state).unwrap();
        let back: AgentRunState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, back);
    }

    #[test]
    fn tc_run_014_done_reason_serializable() {
        for reason in [
            DoneReason::FinalAnswer,
            DoneReason::MaxIterations,
            DoneReason::DegradedToRag,
            DoneReason::NoContext,
        ] {
            let json = serde_json::to_string(&reason).unwrap();
            let back: DoneReason = serde_json::from_str(&json).unwrap();
            assert_eq!(reason, back);
        }
    }
}
