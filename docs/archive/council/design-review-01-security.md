# Phản biện bảo mật cho `remit-design.md` (lần 01)

> Người phản biện: kiến trúc sư bảo mật, có đọc code. Ngày 2026-11-01. Đối tượng: [`../remit-design.md`](../remit-design.md) (K1–K11, U0–U9, STRIDE ở mục 5),
> đối chiếu blueprint [`../ai-run-company-blueprint.md`](../ai-run-company-blueprint.md), `AGENTS.md`, ADR 0004/0009/0017/0018/0021/0022 và code.
> Cách làm: chỉ đọc code và tài liệu; không chạy server, không chạy test, không chạm host thật. Nhãn: **HIGH** = tôi đã đọc đúng dòng code;
> **MEDIUM** = suy ra từ code hoặc ADR, chưa chạy; **LOW** = giả thuyết, kèm phép thử bác bỏ. Không phải tư vấn pháp lý.
> Phạm vi đã đọc: `internal/grant/grant.go` (cả file), `internal/source/gate.go` (cả file), `source/connection.go:205-255`, `source/program.go:1-60`,
> `egress/egress.go:1-120`, `jail/sandbox_darwin.go` (cả file), `jail/plan_linux.go`, `jail/namespaces_linux.go:1-80`, `jail/hidden_linux_test.go`,
> `secrets/secrets.go:1-40`, `childenv/childenv.go`, `auth/local.go`, `clientmcp/handler.go` (phần kiểm loopback), `door/door.go:1-30`,
> `orch/{grants.go:15-230, policy.go:170-260, repo.go:78-386, apply.go:705-730, source_calls.go, agents.go:1-40, library_render.go:15-100}`,
> `agent/{agent.go:70-120, options.go:60-100}`, `library/access.go:80-130`, `cmd/remit/{main.go:200-263, agents.go:60-140}`, `scripts/release.sh:1-35`, `deploy/*`.
> Chưa đọc: `internal/auth/firebase.go`, `orch/session.go` ngoài các dòng token, `orch/tools.go`, toàn bộ `web/` trừ `Markdown.vue`.

## 1. Kiểm từng khẳng định về code ở mục 1–2

| # | Khẳng định của thiết kế | Kết quả | Sự thật (nguồn) |
|---|---|---|---|
| 1 | `grant.Action{Source,Connection,Effect}`, `Effect` chỉ Read/Write (`grant.go:12-16,49-52`) | CONFIRMED | `grant.go:12-17`, `:49-52`. Bổ sung: có **hai** kiểu Effect, `grant.Effect` (uint8) và `source.Effect` (string, `source/gate.go:15-20`); một khái niệm hai chủ |
| 2 | `Decide` thuần, không import module | CONFIRMED | `grant.go:1-8,73-90` |
| 3 | Cổng `pass :138`, `attempt :153`, `permit :178`; chỉ phủ công cụ nguồn, run, script, pull; không biết dừng; lý do là chuỗi tự do | CONFIRMED | `source/gate.go:138,153,178`; `grant.Decision.Reason string` (`grant.go:63-67`) |
| 4 | Quyết định thật là `Decide(grants, Action)` | **WRONG (thiếu)** | Gate gọi `grantPolicy.Decide` → `decideAs(form, capabilities, ref, stored, effect)` (`orch/grants.go:73-86,129-150`): **Form** và **capability ngầm cho đọc** là đầu vào. Chữ ký mới ở mục 4 của thiết kế (`Decide(grants, stops, law, a)`) đánh rơi cả hai |
| 5 | Sổ ghi là một dòng slog (`orch/source_calls.go:19-26`) | CONFIRMED | đúng dòng |
| 6 | `programResolved` không đặt `detail` (argv không vào sổ) | CONFIRMED | `source/connection.go:245` (chỉ script có `detail`, `:255`). ADR-0018 bảng STRIDE nói "records ... the program's argv": **ADR lệch code** |
| 7 | macOS `(allow default)` ở `sandbox_darwin.go:143` | CONFIRMED | `hiddenProfile` `:141-152`. Lưu ý: jail chương trình đã là `(deny default)` + `bsd.sb` (`:123-136`), tức mẫu để sửa phiên đã có |
| 8 | Linux NEWNET/NEWPID chỉ khi `p.Jail != nil` (`plan_linux.go:37-40`) | CONFIRMED | đúng dòng |
| 9 | `agent.Spec` mang ngữ pháp Claude Code (`agent.go:81-99`) | CONFIRMED | đúng dòng |
| 10 | `model.Actor` năm Kind, "rules read Kind, never a name" (`actor.go:14-16`) | CONFIRMED | `actor.go:6-12,16` |
| 11 | Mặc định gắn một công ty: `/workspace/example`, `DEMO`, `.example-secret` | CONFIRMED | `cmd/remit/main.go:92,100,116`; `orch/jira.go:21`; `orch/policy.go:33,126,135` |
| 12 | Pause chỉ đóng phiên có tác vụ nền (`apply.go:719-722`) | CONFIRMED | `orch/apply.go:720-722` (`stopBackground`) |
| 13 | Kiểm tra và lệnh forge chạy ngoài jail | CONFIRMED | `orch/repo.go:97-104` (`repoCommand`: chỉ `childenv`, không `jail`), `runChecks` `:353-375` chạy `go build/vet/test` code agent viết **dưới user máy chủ, không Hide** |
| 14 | `~/.ssh`, `~/.config/glab-cli` đọc được | CONFIRMED | không có trong `protectedFromAgents` (`cmd/remit/agents.go:88-100`); ADR-0018 Consequences |
| 15 | `events` không bao giờ xoá (`store/pack.go:12-14`) | CONFIRMED (lệch dòng) | `store/pack.go:13-17` |
| 16 | 52 migration có checksum | CONFIRMED | `store/migrations.go:3-5,649` ("// 52") |
| 17 | `decidePermission` regex (`orch/policy.go:188`) | CONFIRMED | `:188-223` |
| 18 | Bash/Web mặc định (`orch/agents.go:15`, `library/access.go:92`) | CONFIRMED (gián tiếp) | `agents.go:15` là chú thích; luật thật là `Tools nil = mọi công cụ` (`library/access.go:91-92`) |
| 19 | Mẫu Dispatcher không công cụ (`library/access.go:119-123`) | CONFIRMED | `:118-122` |
| 20 | `provider.Claude` cứng ở `failover.go:156`, `health.go:28` | CONFIRMED | đúng dòng |
| 21 | Danh sách tên cứng `orch/source_tools.go:21`, `env.go:79` | CONFIRMED | `:21` là chuỗi mô tả `list_sources`; `source/env.go:78` `vaultFiles` |
| 22 | `maxGoalChecks=3` (`goal.go:28`) | CONFIRMED | |
| 23 | `blockers` ở `gate.go:157` | CONFIRMED, mơ hồ | Là `internal/orch/gate.go:156-157`, không phải `source/gate.go` mà mục 1 đang dẫn |
| 24 | Bảng KPI duy nhất `outcomes/kpi.go:93` | **WRONG** | `:93` là hàm `share`; bảng là `var groups` ở `outcomes/kpi.go:106` |
| 25 | `attentionOf` (`standing.go:181`) | CONFIRMED | |
| 26 | Hậu tố "only reads" | CONFIRMED | `source/source.go:88-91` |
| 27 | `act` một lần mỗi request id | CONFIRMED | `orch/request.go:42-59` |
| 28 | `DeleteHome` là mẫu xoá | CONFIRMED | `store/homes.go:38` |
| 29 | starrocks ≈400 dòng, bytebase 1.693 | CONFIRMED | 403; 749+201+743 = 1.693 |
| 30 | `release.sh` có `current`/`previous`; `read_change` bảo viết lại `before` bằng tay | CONFIRMED | `scripts/release.sh:5,31-34`; `orch/change_tools.go:50` (còn nhắc `update_mcp_server` đã bị ADR-0017 xoá) |
| 31 | `design.md:38` chứng minh "một tiến trình + `-data` là đơn vị tenant" | **WRONG** | `docs/design.md:38` là non-goal "multi-user"; cờ `-data` gom mọi đường dẫn ở `main.go:203-210,250-263` mới là bằng chứng |
| 32 | "credential không bao giờ vào ngữ cảnh agent" (mục "Đã khớp") | **WRONG một phần** | Đúng cho credential nguồn (ADR-0017). Sai cho đăng nhập model: `childenv.go:29-31` "That login file is the one credential a session holds"; sai cho `~/.ssh`/glab (#14); sai cho token MCP của phiên khác trên macOS (mục 2.2) |
| 33 | "Thư mục dữ liệu nằm trong `Hide` của phiên" | **WRONG (không khả thi)** | `cmd/remit/agents.go:85-86`: thư mục dữ liệu chứa `work/`, `claude/`, `library/` của chính phiên; ẩn nó là phiên không chạy. Phải ẩn từng tệp log, hoặc dời log ra ngoài |
| 34 | `egress` tái dùng với chính sách "cho mọi đích" | **WRONG (không phải tái dùng)** | Không có wildcard: `Config.Allow` là danh sách host:port (`egress.go:62-63`); kiểm dải riêng/loopback gắn với tên trong danh sách (`:12-15`); `MaxBytes`, `Deadline` mỗi tunnel (`:67-70`) sẽ cắt luồng dài của Claude Code. Cần chế độ mới |
| 35 | "OS user mỗi cell đóng ADR-0004" | **WRONG** | ADR-0004 là khoảng hở **agent ↔ máy chủ** (cùng user), không phải công ty ↔ công ty. Xem mục 5 |
| 36 | Refusal `{NoGrant, Ask, Stopped, Law, Rail, Record}` hiện thực I1 | **Lệch blueprint** | Blueprint I1 và 3.7 cho tập đóng `{STOPPED, UNLAWFUL, RAIL, PLATFORM_RECORD}`; `NoGrant`/`Ask` là "least grant" mà mục 7 câu 6 còn để mở. Phải ghi rõ đó là quyết định chưa chốt |
| 37 | `vue-tsc` đỏ 149 lỗi; Feedback "7 bước" | CANNOT CHECK | Không chạy build; không đọc `feedback.go` |

**Tổng: 6 khẳng định sai (#4, #24, #31, #33, #34, #35), 2 sai một phần hoặc lệch (#32, #36), 1 mơ hồ (#23), 1 không kiểm được (#37); các mục còn lại CONFIRMED.**

## 2. Kịch bản tấn công bảng STRIDE bỏ sót

Kẻ tấn công mặc định: **A1** nội dung ngoài lái một agent (ticket, MR, trang web, sự kiện ở `door`); **A2** một agent đã bị lái và có Bash (mặc định hôm nay);
**A3** cell khác trên cùng host; **A4** thành viên có vai trò thấp; **A5** người soạn hoặc đầu độc manifest/pack.

> **Chưa hoàn tất.** Bản này dừng sau mục 1. Các mục 2–7 (kịch bản tấn công theo K1/K2/K3/K5/K7/K9/K10, bề mặt tấn công mới, điểm cố định trên Linux/macOS, OS user mỗi cell, mười thay đổi, điều làm đúng) chưa được viết.
