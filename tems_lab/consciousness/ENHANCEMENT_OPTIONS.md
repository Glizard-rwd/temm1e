# Consciousness Enhancement Options

> Based on findings from v1 report, v2 experiments (54 runs), budget fix analysis, and architecture review.
> All options are **model-agnostic**, **timeless**, and **objectively measurable**.

---

## The Core Problem

Consciousness currently observes but cannot act beyond injecting text. It sees everything — budget, tools, drift, failures — but its only output channel is a system prompt whisper. This is like giving a flight controller a radio but no ability to change the flight plan.

---

## Option A: Consciousness-Gated Budget (Zero Code Risk)

**What:** Consciousness checks the budget before EVERY provider.complete() call. If spend is within 10% of the limit, consciousness decides whether this call is worth it — not just whether the budget technically allows it.

**Why it's objectively better:** Currently, budget check is a simple threshold. Consciousness has context (is this a retry? are we spinning? is the user's question already answered?) that a threshold doesn't. A $0.05 call is wasteful if the answer was already computed. A $0.05 call is essential if the agent is mid-tool-loop on a critical task.

**Measurable:** Compare sessions-that-hit-budget-limit before/after. Consciousness should reduce budget-exceeded events by allowing smarter spend allocation.

**Scope:** ~20 lines. Add `check_budget_with_context()` that passes the budget state + turn context to consciousness. If consciousness says "STOP: answer already sufficient", skip the call.

---

## Option B: Retry Loop Detection + Strategy Rotation (Trajectory-Level)

**What:** Consciousness tracks consecutive failures in its session_notes. After N consecutive tool failures or LLM non-answers, consciousness injects a **strategy rotation** — not just an observation, but a concrete directive: "Try a different tool", "Simplify the approach", "Ask the user for clarification."

**Why it's objectively better:** The current agent retries the same approach until it either works or exhausts budget. An external observer with full trajectory visibility can detect spinning before the agent does (the agent only sees the current turn; consciousness sees the pattern across turns). This is the "fix-test-fix loop" scenario from V4 — but done right.

**How it's different from current:** Current consciousness says "the agent seems to be struggling." Enhanced consciousness says "STRATEGY: You've failed 3 times with shell_exec. Switch to file_read + manual parsing." The injection becomes prescriptive, not descriptive.

**Measurable:** Count retry-loops-broken per session. Compare total cost of sessions with retry-heavy workloads (tool errors, API failures, ambiguous tasks).

**Scope:** ~50 lines. Add `failure_tracker` to ConsciousnessEngine. When consecutive_failures >= 3, pre_observe generates a strategy rotation directive instead of a generic observation.

---

## Option C: Context Window Pressure Relief (Memory-Level)

**What:** When the conversation history approaches the context window limit, consciousness decides what to keep and what to summarize. Currently, the context builder prunes mechanically (oldest messages first). Consciousness can prune semantically — keeping the user's original intent, the key decisions, and the current plan, while compressing verbose tool outputs and intermediate reasoning.

**Why it's objectively better:** Mechanical pruning loses critical context (the user's first message often contains the real intent). Semantic pruning preserves trajectory information that a turn-by-turn agent would lose. This is the one place where consciousness has information the primary agent structurally cannot have — it maintains session_notes that survive context pruning.

**Measurable:** On 20+ turn conversations, compare "does the agent remember the original intent?" (human judge) and "does the agent contradict earlier decisions?" (automated check). Also compare token waste — semantic pruning should use fewer tokens for the same effective context.

**Scope:** ~100 lines. Add `consciousness_prune()` that receives the full history and returns a compressed version. Called when history exceeds 80% of context budget. Uses consciousness LLM call with prompt: "Summarize this conversation preserving: (1) user's original request, (2) decisions made, (3) current plan, (4) unresolved issues."

---

## Option D: Tool Call Validation (Pre-Execution Gate)

**What:** Before every tool execution, consciousness gets a veto. It sees the tool name, arguments, and the full trajectory of what led to this call. It can approve (silent), warn (inject note), or block (return error to agent without executing).

**Why it's objectively better:** The agent decides tool calls from its current turn context. Consciousness sees the full session — it can catch: (1) destructive operations the user didn't ask for, (2) redundant tool calls (reading a file already read), (3) tool calls that contradict the user's intent, (4) infinite tool loops. This is the "safety layer" that consciousness was architecturally designed for.

**Measurable:** Count blocked-tool-calls and validated-tool-calls. Compare "unnecessary tool calls per session" before/after. Compare user-reported "agent did something I didn't want" incidents.

**Scope:** ~80 lines. Add `validate_tool_call(tool_name, args, context)` to ConsciousnessEngine. Returns Approve/Warn/Block. Called in runtime.rs before `tool.execute()`. Uses a focused consciousness prompt: "Should this tool call proceed? Context: [user intent], [previous tools], [current plan]."

---

## Option E: Adaptive Activation (Cost-Proportional)

**What:** Instead of consciousness firing on every turn (current behavior), consciousness activates proportionally to task complexity. Simple chat → no consciousness. Order with tools → pre-observe only. Complex multi-tool task → full pre+post. This is decided by the V2 classifier output that already exists.

**Why it's objectively better:** Current consciousness fires on every Order turn regardless of complexity. This wastes ~$0.002/turn on turns where the agent doesn't need help (the 100% accuracy results prove this). Adaptive activation preserves consciousness for the turns that actually benefit — long tool chains, high-stakes operations, late-session turns where drift is likely.

**Measurable:** Compare total consciousness cost per session. Should decrease 40-60% while maintaining the same (or better) accuracy. The V2 classifier already outputs difficulty (Simple/Moderate/Complex/Expert) — use it.

**Scope:** ~30 lines. In runtime.rs, check `classification.difficulty` before calling `pre_observe()`. Only fire consciousness when difficulty >= Moderate. Always fire post_observe (it's cheap and provides trajectory continuity).

---

## Option F: Consciousness Memory Persistence (Cross-Session)

**What:** Currently, consciousness session_notes reset when the session ends. Enhanced: consciousness writes a "session summary" to the memory backend at session end. On next session start, consciousness reads the previous summary. This gives consciousness cross-session trajectory awareness.

**Why it's objectively better:** Users return to ongoing projects. The agent starts fresh each session; consciousness could carry forward: "Last session, the user was building a REST API. They got stuck on authentication. They prefer explicit error messages over silent failures." This is information that lambda-memory might store as raw facts, but consciousness can store as trajectory-level insights — what matters for the agent's behavior, not just what was said.

**Measurable:** On returning-user sessions, compare "turns to productive context" (how many turns before the agent understands what the user is working on). Should decrease significantly with cross-session consciousness.

**Scope:** ~60 lines. At session end, consciousness makes one final LLM call: "Summarize this session in 3-5 bullet points for your future self. Focus on: user's project, unresolved issues, user's preferences." Store in memory backend with key `consciousness:session_summary:{chat_id}`.

---

## Ranking by Impact vs Effort

| Option | Impact | Effort | Risk | Recommendation |
|---|---|---|---|---|
| **E: Adaptive Activation** | High | Low (~30 LOC) | Zero | **DO FIRST** — pure cost optimization, no behavior change |
| **B: Retry Loop Detection** | High | Low (~50 LOC) | Zero | **DO SECOND** — addresses the one proven failure mode (V4 OrderFlow) |
| **A: Budget-Gated Calls** | Medium | Low (~20 LOC) | Low | DO THIRD — safety improvement |
| **D: Tool Call Validation** | High | Medium (~80 LOC) | Low | DO FOURTH — biggest safety win |
| **C: Context Pressure Relief** | High | Medium (~100 LOC) | Medium | DO FIFTH — needs careful testing on long sessions |
| **F: Cross-Session Memory** | Medium | Medium (~60 LOC) | Low | DO SIXTH — needs multi-session test harness |

---

## Non-Options (Rejected)

**"Make consciousness smarter with a bigger model"** — Violates model-agnostic. Consciousness should work with whatever model the user configures. The observer should be small and cheap, not a second full-power agent.

**"Add more observation points"** — Current 2 (pre+post) are sufficient. More observation points = more cost for diminishing returns. The Phase 2 results show consciousness already fires ~8 events/run on simple tasks.

**"Let consciousness modify the conversation history"** — Violates the observer principle. Consciousness observes and advises; it never acts directly. Letting it edit history creates a two-agent coordination problem that's harder than the one it solves.

**"Train consciousness on past sessions"** — Violates timelessness. Fine-tuning is provider-specific, expensive, and creates drift. The system prompt approach (inject observations into context) works with any model.
