# Phản biện thiết kế remit, 02: kiến trúc và quy ước

> Người phản biện: kiến trúc và quy ước. Đối tượng: [`docs/remit-design.md`](../remit-design.md) (K1–K11, mục 3, mục 4, U0–U9).
> Đối chiếu với `AGENTS.md`, `docs/adr/*`, `docs/design.md`, `internal/fitness/*` và code thật, ngày 2026-11-01. Chỉ đọc, không sửa.
> Nhãn: **HIGH** = tôi đã đọc đúng dòng code hoặc chạy lệnh; **MEDIUM** = suy ra từ code đã đọc; **LOW** = đoán.
> Lệnh đã chạy: `go test -count=1 -run 'TestEveryADRLinkResolves|TestTheDocsNameEveryFlagAsItIs|TestEveryImportBetweenModulePackagesIsAnAllowedEdge' ./internal/fitness/` → `ok 1.158s` (HIGH).
> Đếm nơi gọi bằng `rg -n <mẫu> --type go` (tổng) và thêm `-g '!*_test.go'` (prod).

## 0. Kết luận ngắn

Hướng đi đúng ở ba chỗ lớn: cell theo tiến trình (K1b), đưa mọi đường ra qua một cổng (K2), và dừng đo được (K3). Phần còn lại
**thêm chủ thứ hai cho những khái niệm đã có chủ**: 13 bảng mới (mục 3), trong đó ít nhất 8 bảng trùng một bản ghi, một cờ hay
một trạng thái đã có. Thiết kế không nhắc ADR-0003 (cấm thực thể mới) và để khôi phục (U9) **cuối cùng**, ngược với
`AGENTS.md:21-22` ("The recovery ... is built before the risk is taken"). Phác thảo `grant.Decide` ở mục 4 bỏ mất `Form` mà
`decideAs` cần (`orch/grants.go:73`) và đặt vào `Refusal` hai giá trị không thể là đầu ra của một hàm thuần (`Ask`, `Record`).
K6 không tồn tại: mục 2 nhảy K5 → K7, còn "K6" chỉ là một gạch đầu dòng trong K5 (`remit-design.md:113`).

| K | Phán quyết | Lý do một dòng |
|---|---|---|
| K1 | ACCEPT cell; SIMPLIFY `settings`, `members` | Cell đã có cờ riêng; `members` trùng `-google-emails`; Assistant một người chưa được giải |
| K2 | ACCEPT phủ rộng và sổ trong DB; SIMPLIFY `Class`, `Refusal`, `Params`, chữ ký `Decide`; DEFER chuỗi băm | Xem mục 3, 5 |
| K3 | ACCEPT mức dừng và `Drill`; SIMPLIFY `stops` | `stops` trùng `ChainPaused`, `CardStopped`, `Paused`, `outages` |
| K4 | SIMPLIFY judge; DEFER worker | Judge không công cụ không làm được việc judge hôm nay; trùng tên `orch.Judge` |
| K5 | ACCEPT đường ghi, Jira thành Provider; SIMPLIFY manifest; DEFER `compute` | Dạng stdio MCP đã là `run` (ADR-0018) |
| K7 | SIMPLIFY mã lý do, trần vòng; DEFER tập niêm, `probes`, `runs`, `anchors` | `FindingBlocker` đã là tập đóng; trần đã có |
| K8 | SIMPLIFY | `Release` đã là `vcs.revision` (`cmd/remit/health.go:30-41`); `Hash` là dữ liệu dẫn xuất |
| K9 | SIMPLIFY | `Signature` trùng `Change.ApprovedBy` + câu trả lời inbox |
| K10 | DEFER, trừ giữ lại theo mẫu `DeleteHome` | Pack là dữ liệu nhúng, không phải bảng; `access_log` là một view |
| K11 | ACCEPT, và đưa lên đầu | Recovery-first |

## 1. Đối chiếu ADR

`TestEveryADRLinkResolves` (`internal/fitness/adr_test.go:54-86`) quét `AGENTS.md`, `README.md`, `docs/*.md`, `docs/adr/*.md`:
mọi `ADR-NNNN` phải có file, và mọi `Enforced by:` trong một ADR phải gọi một hàm `Test…` đang có. `docs/remit-design.md`
nằm trong glob `docs/*.md`, nên **viết `ADR-0025` vào nó trước khi file ADR tồn tại sẽ làm đỏ build** (HIGH). Hôm nay nó chỉ trỏ
tới các ADR có thật, và test qua. Mỗi ADR mới chỉ được ghi `Enforced by:` các test có mặt trong cùng thay đổi; test viết trước
(`AGENTS.md:88`). Mục lục `docs/adr/README.md` không bị test kiểm, phải sửa tay; dòng trạng thái của ADR cũ đổi theo
`docs/adr/README.md:3-5`.

| K / U | ADR bị thay (S), thu hẹp (N), mâu thuẫn (C) | Câu ADR mới phải nêu, có trích dẫn |
|---|---|---|
| Mọi K | **ADR-0003 (C)**: "No new persisted entity ... until ADR-0001 and ADR-0002 have held for four weeks" (`0003:13-14`). ADR-0002 đã bị 0010 thay, nên điều kiện không bao giờ xảy ra; các bảng sau đó (`feedback`, `subscriptions`, `chain_grants`, `placements`; `migrations.go:486,495,612,669`) đã sống ngoài nó | "Supersedes 0003: a new entity is held by *Follow the pattern for that kind of thing*, not by a wait." Không có câu này, 13 bảng mới mâu thuẫn một ADR đang `accepted` |
| K1 | **ADR-0004 (S)** toàn bộ (OS user riêng là "option 2"; `0004:20-27` nói đó là việc của host, ngoài repo). **ADR-0022 (N)**: "A local server has one owner, `local`, and no list of accounts" giữ nguyên cho `local`. **ADR-0006 (N)**: "Sessions use the machine's Claude Code login" thành "the cell user's login". **ADR-0010 (C, điều kiện xét lại)**: "Revisit when ... runs for someone other than its owner" (`0010:43`): nhiều công ty chính là điều kiện đó | Một ADR "a tenant is a cell" ghi: supersedes 0004; narrows 0006 (login theo user của cell), 0022 (chế độ local một owner); và một đoạn trả lời câu xét lại của 0010 (giữ "no cost" hay không) |
| K2 | **ADR-0018 (N)**: bất biến 3 ("records what was asked") nay ghi vào DB; mục *Revisit when* "a rule more specific than `(source, connection, effect)`" (`0018:122-123`) chính là K2. Câu "the session keeps the network it needs to reach its model provider" (`0018` mục *Where the executor runs*) bị thay bởi proxy. **ADR-0015 (N)**: "read-only is enforced by the code of each tool" | "Narrows 0018: an action carries its effect on an ordered scale ...; the record of calls is a table." Không cần ReBAC |
| K2 (Hide) | **ADR-0012 (C)**: playbook `self-development` bảo agent đọc `~/.remit/agents.log` (`internal/orch/playbooks.go:72`). Giấu log khỏi phiên phá playbook đó. `-data` cũng chứa thư mục làm việc của chain (`cmd/remit/main.go:90`): giấu cả thư mục dữ liệu là giấu worktree | ADR mới phải nói log nào giấu, log nào phiên đọc được, và sửa brief `self-development` trong cùng thay đổi |
| K3 | **ADR-0021 (N)**: các hệ quả "a `setsid` child survives", "the relay's port is on the host's loopback" (`0021` *Consequences*). **ADR-0004 (S)** như K1 | Liệt kê lại từng hệ quả của 0021 còn đúng hay đã đóng, kèm test đo |
| K4 | **ADR-0006 (S một phần)**: "Every agent runs on Claude Code" và "No API key is set" (`0006:32-35`); phương án 2 bị bác vì "an abstraction with one member is a flag in disguise" (`0006:24-25`). **ADR-0016 (N)** và **ADR-0013**: "A result is confirmed by the goal judge's last word" (`Approval.Confirms`, `internal/model/model.go:175-179`). **ADR-0010 (C)**: judge qua HTTP API là "metered pricing" (`0010:43`). Worker shim: **ADR-0007** | "Supersedes in 0006: judges may run on other providers through ...; the credential of a judge is a source connection (ADR-0017), not a flag." Và một câu nói ai xác nhận kết quả: `Confirms` hay `Panel` |
| K5 | **ADR-0017 (S một phần)**: "Agents hold only tools Remit writes ... Extending ... means writing a source: one file in `internal/source` and one line in `kinds`". **ADR-0018 (C)**: "A program is a CLI, a script or an MCP server on stdio: one concept, `run`" (`0018` *Forms*); K5 làm stdio MCP thành dạng thứ ba | "Supersedes in 0017: a source may be declared as data, applied by the owner; its endpoints are typed tools behind the gate." Bỏ dạng stdio MCP khỏi manifest |
| K7 | ADR-0013, ADR-0016 (N): mã lý do vào `Finding` | Nêu quan hệ với `FindingBlocker` (`model.go:193-201`) |
| K8 | Không ADR nào; `design.md` mục Model (Change) | — |
| K9 | **ADR-0019 (N)**: dải Results hôm nay có Outcomes, Feedback và "each entry counts what waits in its own area" | Nêu tab mới (nếu còn) và cái nó đếm; dưới 6 tab (`0019:81`) |
| K10 | `design.md` và `store/pack.go:12-17` ("never deleted: the design keeps every session's record"); ADR-0024 (placement giữ một tuần) là tiền lệ xoá | ADR giữ lại theo lớp phải thay câu "never deleted" ở `pack.go` và `design.md` |
| U0 | ADR-0024 tự nói "go in the same change as their replacements" (`0024:88-90`): U0 là nợ của chính thay đổi ADR-0024, không phải việc mới | Theo *Ownership*: báo cho chủ phiên đó, không nhận làm phần của remit |

## 2. Một khái niệm, một chủ: các chủ trùng

| Mới | Chủ đã có | Chứng cứ | Đề xuất |
|---|---|---|---|
| `stops(scope,id,by,at)` | `ChainPaused` (chỉ `step` đặt), `CardStopped`, `Subscription.Paused`, `EventSignal.Paused`, `ClientKey.Paused`, `outages` (dừng provider) | `model.go:15,266`; `signal.go:117`; `event.go:157`; `client_key.go:19`; `orch/health.go:39-52`; `TestOnlyStepSetsAChainsStatus` (`writers_test.go:136`) | `StopView` là hàm thuần dẫn xuất từ các chủ đó; chỉ thêm một chủ mới: dừng cả cell |
| `Refusal.Ask` | `grant.Ask` | `grant.go:58-60` | `Refusal` chỉ nói *vì sao*; `Verdict` nói *xử thế nào* |
| `Refusal` (tập đóng) | `FindingBlocker` (tập đóng của judge), `signal.Reason`, `egress.Denial.Reason` | `model.go:193`; `signal/evaluate.go:40`; `orch/program.go:131` | Đặt tên theo tầng: `grant.Refusal`; không dùng lại cho `Finding` |
| `Class` trên `Action` + "bảng của nền tảng" | `source.Tool.Effect` khai trong công cụ; `grant.Effect` có thứ tự | `source/source.go:80-81`; `grant.go:10-16` | Mở rộng thang `Effect` thay vì thêm trường và bảng thứ hai |
| `source.Action` / `grant.Action` | Hai `Action`, hai `Effect`, hai bộ `Verdict` đã có, dịch nhau ở `policyVerdict` | `source/gate.go:43-63`; `grant.go:49-68`; `orch/grants.go:106-115` | K2 chỉ sửa `grant.Action` (mục 4): sẽ thành ba chỗ phải giữ đồng bộ. Đổi cả hai, hoặc đặt trường mới ở `source.Action` rồi dịch một lần |
| `settings` (cờ làm hạt giống) | 36 cờ trong `cmd/remit/main.go`; `TestTheDocsNameEveryFlagAsItIs` buộc README nêu từng cờ | `main.go` (`rg -c 'flag\.'` = 36); `fitness/flags_test.go:93` | Cell = một đơn vị launchd/systemd với cờ riêng: cấu hình theo cell đã là cờ. Không có bảng |
| `members(id,who,roles)` | `-google-emails` ("accounts that may sign in") | `main.go:123` | Mở rộng cờ thành `email=role`; một chủ của "ai được vào" |
| `Actor.Role` + bảng hành động→vai trò | `act` kiểm `by.Kind != ActorOwner` theo `method`; `ForOwner()` cho Assistant quyền owner; 9 chỗ `Kind == ActorOwner` | `orch/request.go:47-54`; `model/actor.go` (`ForOwner`); xem mục 3 | Bảng vai trò là `allowed` của `once`, khoá theo `method`; xoá các phép so `Kind` rải rác |
| `gate_calls` | `callLog` (dòng slog) và sự kiện công cụ trong `events` ("the tool event in the agent's feed holds the arguments") | `orch/source_calls.go:10-25` | Thay `callLog`, không chép đối số (thiết kế đã chỉ giữ `params_digest`: đúng). Tên theo `source.Call`: bảng `source_calls` |
| `gate_calls.cell`, `stops.scope=cell` | K1(b): một cell một DB | `remit-design.md:57-59` | Bỏ cột `cell`: mọi hàng có cùng giá trị là cờ trá hình |
| `access_log` | `gate_calls` có `data` | `remit-design.md:78,164` | Là truy vấn trên sổ gọi theo lớp dữ liệu, không phải bảng |
| `purposes`, `retention` | Nội dung của pack do luật sư ký: dữ liệu tĩnh | `remit-design.md:138-140` | Nhúng vào binary như playbook dựng sẵn (`internal/builtin`), đổi qua MR |
| `Signature` | `InboxChange` trả lời bằng Apply, `Change.ApprovedBy`, `By` của sự kiện | `model.go:520-525`; `model/change.go:52-54` | Chữ ký là câu trả lời của owner cho một item; thêm băm câu vào item |
| `Release` (trên `Change`, "gắn vào mỗi Card") | `vcs.revision` mà `/healthz` báo; playbook id `release` ("Release train") | `cmd/remit/health.go:30-41`; `orch/playbooks.go:90` | Ghi bản dựng vào bản ghi khởi phiên; đặt tên khác `release` (ví dụ `Build`) |
| `Change.Hash` | `Change.After` (JSON) | `model/change.go:40-56` | Dẫn xuất, không lưu |
| `findings (cột mới)` | Không có bảng `findings`: `Finding` là JSON trong `approvals.doc` | `migrations.go:174`; `rg 'CREATE TABLE.*findings'` = 0 | Trường JSON `omitempty`, không cần migration |
| `Finding.Reason` | `Finding.Blocker` (tập đóng, kiểm ở `findingsError`) | `model.go:183-201`; `orch/gate.go:114,157` | Nếu cần lý do cho `not_met`, thêm giá trị vào tập đóng hiện có, cùng hàm kiểm |
| `probes` + "một loại decision" | Thiết kế ghi tỷ lệ bắt ở hai nơi | `remit-design.md:119,160` | Chọn một |
| `anchors`, `runs` | Chain đã xong có goal, MR, `Approval`, kết quả watch (ADR-0013) | `model.go:160-179` | Neo đầu tiên dẫn xuất từ merge của owner và watch; `runs` hoãn |
| `internal/judge.Judge`, `Verdict` | `orch.Judge`, `orch.Verdict` (`verification.go:63,77`), `grant.Verdict`, `source.Verdict`, `Approval.Verdict`, `wire.Verdict` | `rg 'type .*Verdict'` | Sáu "Verdict" đã có; gói mới không được thêm cái thứ bảy cùng tên khác nghĩa |
| Egress proxy cho phiên (K2 ii và K3) | `egress.Gate`: allowlist host:port do người gọi cố định, deadline và giới hạn byte mỗi tunnel | `internal/egress/egress.go:1-18`; `fitness/imports_test.go` mục `internal/egress` | Phiên streaming dài sẽ chạm deadline/giới hạn byte. Một kiểu riêng (đo và cắt), không phải chế độ "cho mọi đích" của `Gate` |

## 3. Phác thảo mục 4 so với chữ ký thật, và số nơi gọi

**`grant.Decide(grants, stops, law, a)`** (`remit-design.md:174`) so với `grant.Decide(grants []Grant, a Action) Decision` (`grant.go:73`):
- `Form` mất khỏi `Action` mới, nhưng quy tắc chọn theo form nằm ở `decideAs(form, …)` (`orch/grants.go:73-86`): ổn nếu `Form` vẫn ở
  `source.Action` (`gate.go:43-47`). Phác thảo phải nói rõ.
- `Refusal.Record` ("không ghi được vào sổ") là kết quả I/O của vỏ: một hàm thuần không biết lần ghi sau nó có thành không. Trong
  `pass` (`gate.go:138-151`), `Recorder.Record` chạy **sau** `attempt`, và `Recorder` "cannot refuse a call" (`gate.go:96-100`). Muốn
  "không ghi được thì không chạy" là đổi thứ tự `pass` (ghi trước, rồi `act`), không phải thêm một giá trị `Refusal`.
- `StopView`, `LawView` mà `grant` import từ `internal/policy` sẽ phá `"internal/grant": {nil, …}` (`fitness/imports_test.go`).
  Nếu `policy` trả `Refusal.Law` thì `policy → grant`. Nhỏ hơn: giữ nguyên `grant.Decide`, ghép dừng và luật ở lõi thuần
  `orch/grant_access.go` (đã trong `pureCore`, nơi đã ghép capability và kết nối protected).
- `Decision{Verdict, Refusal}` bỏ `Reason`: mất câu agent đọc, mà `gate.permit` cần (`gate.go:195,197`).
- Thiết kế nói phải bỏ "chuỗi lý do làm khoá" (`remit-design.md:73,83`). `rg 'Reason ==|switch .*Reason|Contains(.*Reason'`: không chỗ
  nào so `grant.Decision.Reason` như khoá (chỉ có `signal.Reason`, `egress.Denial.Reason`). Lý do cho `Refusal` là đo được (M2),
  không phải sửa một lỗi có thật.

**`source.Policy`** (`gate.go:66-68`): một cài đặt prod (`grantPolicy`, `orch/grants.go:129`), hai cài đặt test (`grants_test.go:40`,
`gate_test.go:41`). **`agent.Launcher`/`Spec`** (`agent/agent.go:81-118`): một cài đặt prod (`ClaudeLauncher`, `agent/claude.go:44`)
và một fake (`orch/orch_test.go:140`); `Spec` trung lập cần cả `claudecode` dịch lại.

| Thay đổi | Ký hiệu đếm | prod / tổng | Nơi chính |
|---|---|---|---|
| Chữ ký `grant.Decide` | `grant\.Decide\(` | 3 / 3 (+4 trong `grant_test.go`) | `orch/grant_access.go:40,54,73` |
| | `grant\.Action\{`, `grant\.Decision\{` | 1 / 1; 7 / 7 | `grant_access.go:35,50,52`; `grants.go:85` |
| `Reason` → `Refusal` | đọc `.Reason` của quyết định | 8 | `gate.go:195,197`; `grants.go:111,113,217,220`; `grant_access.go:56,67` |
| | `source\.Verdict`; `VerdictAllow\|Deny\|Ask` | 15 / 28; 14 / 35 | |
| `Class` / thang hiệu ứng | `NewTool\(` (công cụ phải xếp lớp) | 34 / 37 | 34 công cụ + `run` + `script` |
| | `EffectRead\|EffectWrite`; `grant\.(Read\|Write)` | 8 / 26; 5 / 26 | |
| Vai trò | `act`/`actDone(ctx, o, req`; `once(ctx, o,` | 28; 2 | `orch/request.go:47,59,124` |
| | `Kind (==\|!=) model.ActorOwner`; `ForOwner()` | 9; 7 / 8 | `client_keys.go:273`, `change_item.go:97,249`, `credentials.go:185,231`, `grants.go:316`… |
| `stops` thay đường dừng | `ChainPaused`; `CardStopped`; `.Paused` | 20 / 49; 18 / 60; 20 / 57 | U3 "xoá đường dừng riêng lẻ" là chừng ấy chỗ cộng test của chúng |
| `Spec` trung lập (K4.2) | `agent\.Spec\{`; `AllowedTools`; `DenyTools` | 4; 5 / 10; 4 / 15 | `intake.go:226`, `access.go:59,66`, `health.go:344` |
| Provider theo tên | `provider\.Claude` | 12 / 67 | `failover.go:57-208`, `health.go:28-295` |
| Bỏ Bash/Web mặc định | `"Bash"\|Bash(\|"WebFetch"\|"WebSearch"` | 18 / 100 | và 8 playbook dựng sẵn (`orch/playbooks.go:17-90`), mọi playbook viết code cần lại `Bash` |
| Sổ thay `callLog` | `callLog` | 5 / 6 | `source_calls.go`, `orch.go:142` |
| Jira thành Provider | `jira` | 14 / 35 | `orch/jira.go:21` (`DEMO`) |
| `Finding` mới | `model\.Finding\{` | 1 / 36 | `findingsError` (`orch/gate.go:114`) |
| `Change` mới | `model\.Change\{` | 1 / 4 | `read_change` (`orch/change_tools.go:50`; còn nhắc `update_mcp_server`, công cụ mà ADR-0017 đã xoá: lỗi ngoài phạm vi, báo lại) |
| Giấu thư mục dữ liệu | `HiddenWith\(` | 2 / 12 | `cmd/remit/agents.go:20-21` |

Cỡ "do it all at once" (`AGENTS.md:23-25`): U2 như đang viết chạm khoảng 100 chỗ prod (cộng các dòng trên) trong `grant`, `source`,
`orch`, `jail`, `egress`, `cmd`, cộng 8 playbook và test của chúng. U3 chạm thêm khoảng 58 chỗ trạng thái dừng. Mỗi đơn vị vẫn
làm được trong một thay đổi, nhưng U2 phải tách đúng ranh giới (mục 4), nếu không nó thành một thay đổi trên khoảng 10 gói (MEDIUM).

## 4. Thứ tự U0–U9: vòng và tiền đề ẩn

- **Vòng U2 ↔ U3.** `Decide` của U2 trả `Stopped` đọc `stops` (K2, `remit-design.md:74`), mà `stops` thuộc U3, và U3 phụ thuộc U2.
  Egress proxy cũng nằm trong cả U2 và K3. Gỡ: `StopView` dẫn xuất từ trạng thái có sẵn (thay đổi 6) thì U2 không cần U3; proxy
  thuộc U3.
- **Khôi phục cuối cùng.** U9 (`store.Restore`, sao lưu ngoài đĩa) phụ thuộc mỗi U1 nhưng xếp cuối. `AGENTS.md:21-25` đòi
  rollback, dừng theo tenant và backup xây **trước** rủi ro. U9 và phần dừng của U3 phải đi trước U2, U4, U7.
- **U1 không cần cho U2**, chỉ vì cột `cell` thừa. Bỏ cột thì U2 độc lập.
- **U8 thiếu U1 và U7.** Chữ ký của `officer` cần vai trò (U1); "danh mục hồ sơ cần chữ ký" ở trong pack (U7).
- **U5 thiếu nguồn credential.** Judge qua HTTP cần khoá API. Chủ duy nhất của credential là `source_values` qua item inbox
  (ADR-0017), nên U5 phụ thuộc đường credential của nguồn (U4 hoặc cái đã có). `n_eff` cần `anchors` trước `Panel`.
- **Mạng của phiên (U2/U3) không có tiền đề.** Phiên tới MCP của Remit qua loopback (`cmd/remit/mcpurl.go:8-19`) và
  tới nhà cung cấp model (ADR-0018). Namespace mạng chỉ đến proxy, hay Seatbelt `network-outbound` chỉ đến cổng proxy, sẽ cắt cả
  hai, trừ khi relay chúng. Thiết kế không nhắc.
- **OS user mỗi cell cần quyền root** để tạo user (`useradd`, `sysadminctl`). ADR-0004 gọi đó là "host work outside this
  repository" (`0004:24-27`). Bộ điều khiển thành thành phần có đặc quyền: threat model ở mục 5 chỉ ghi I/E cho nó, thiếu E qua
  chính bộ điều khiển.
- **Bỏ Bash mặc định trước khi có capability `shell`** làm hỏng mọi chain viết code. Capability phải có trong cùng U2.
- **K10 ↔ K2 xung đột.** Xoá theo lớp giữ lại trên một sổ băm chuỗi làm gãy chuỗi, trừ khi neo theo đoạn. Phải quyết cùng nhau.
- **Assistant với nhiều người** (U1). Hôm nay có một Assistant, một hội thoại (`docs/design.md:851-857`), và `ForOwner()` cho mọi act
  của Assistant quyền owner. Một `viewer` nói với Assistant sẽ được quyền owner. Đây là tiền đề của bất kỳ vai trò nào.

## 5. Từng K: thay đổi nhỏ nhất giải đúng gốc, hay tổng quát hoá phỏng đoán

- **K1.** Gốc: mặc định gắn một công ty và không có đơn vị dừng tenant. Cell giải đúng (ACCEPT). `settings` là phỏng đoán:
  đơn vị theo cell đã mang cờ. `members`/vai trò không có người dùng thứ hai và chưa giải Assistant: SIMPLIFY thành cờ, kèm một
  quyết định về Assistant. Router HTTP "tuỳ chọn, sau" (`remit-design.md:61`): xoá câu, vì không có cái gì "sau" (`AGENTS.md:23-25`).
- **K2.** Gốc: có đường ra ngoài cổng, và sổ chỉ là log. Phủ rộng và sổ trong DB đúng (ACCEPT). `Class` là trường thứ hai cho cái
  `Effect` đã mô hình (SIMPLIFY). `Params[].origin`: cổng không thể biết nguồn gốc của một giá trị agent tự viết vào input (`gate.go:21-23`,
  "the tool's own input is the agent's to write"), nên không hiện thực được như mô tả. Theo dõi theo card (card đã đọc nội dung
  ngoài) thì làm được và xác định (SIMPLIFY). Chuỗi băm và neo ngoài: phiên đã không đọc được DB (ADR-0018, `TestTheOwnerKeyIsHiddenFromAgents`
  cùng lớp), nên chuỗi băm chống một kẻ chưa có mặt, và xung đột với giữ lại. DEFER tới khi sổ ra khỏi ranh giới (xuất cho kiểm toán) (MEDIUM).
- **K3.** Gốc: dừng không đóng mọi phiên, con `setsid` sống sót. Mức dừng và `Drill` đo bằng mã: ACCEPT. Bảng `stops`: SIMPLIFY.
  Dừng provider đã có (`outages`, `orch/health.go:39-52`).
- **K4.** Gốc: judge cùng họ model. Nhưng judge hôm nay là card có công cụ, phải cho `file:line` và đầu ra lệnh (`model.go:181-190`).
  Một judge HTTP không công cụ chỉ chấm được artifact được đưa cho nó: nó là phiếu thứ hai, không thay judge. SIMPLIFY: đặt tên
  `Rater`/`internal/rate`, ghi kết quả thành `Approval` (một chủ của "xác nhận"), credential là kết nối nguồn. Worker: DEFER (thiết kế đã hoãn).
- **K5.** Gốc: thêm hệ thống cần Go. Manifest REST là dạng T1 khai báo (Injector đã có endpoint "named as reads",
  `egress.go:20-30`): ACCEPT với một dạng. stdio MCP: bỏ (đó là `run`). `compute`: DEFER, chưa có người gọi, và K6 không có văn bản.
- **K7.** Trần vòng đã có (`maxGoalChecks = 3`, `orch/goal.go:28`): mở rộng cho gate review/qc là đủ. Tập niêm, `probes`, `runs`:
  DEFER, chưa có tiêu thụ nào trong `outcomes` hôm nay. Bảng KPI duy nhất là `groups` ở `outcomes/kpi.go:106`, không phải
  `kpi.go:93` như thiết kế trích (`:93` là thân của `share`).
- **K8.** `revert_change` thay hướng dẫn viết lại bằng tay: ACCEPT. `Hash`, `Release` trên `Change`: SIMPLIFY. Cờ rủi ro pháp lý: DEFER cùng K10.
- **K9.** Sắp hàng đợi theo hiệu ứng trong `attentionOf` (`orch/standing.go:181`, lõi thuần): ACCEPT. `Signature`: SIMPLIFY.
- **K10.** Pack chưa có luật sư ký, theo chính thiết kế (`remit-design.md:139`): DEFER. Giữ lại theo lớp: ACCEPT, theo mẫu quên
  placement sau một tuần qua `DeleteHome` (`store/placement.go:62-72`). Lưu ý `TestAMigrationNeverRewritesEvents` cấm migration
  xoá events, không cấm job lúc chạy; nhưng `store/pack.go:12-17` và `design.md` hứa "never deleted", phải viết lại.
- **K11.** ACCEPT, đưa lên đầu. `/metrics` không được mang chỉ số chi phí (ADR-0010).

## 6. Biểu mẫu, màn cài đặt, và trạng thái trùng giữa server và browser

- `settings` không nói đổi bằng gì. Nếu bằng màn: phạm `AGENTS.md:39-41`. Nếu bằng agent: các cờ là rail (`-max-waiting-mrs`, ADR-0001)
  sẽ do agent đổi, phạm `AGENTS.md:50-52`. Lý do thứ ba để bỏ bảng.
- "Mỗi bản phát hành mang một trường có/không về rủi ro pháp lý" (`remit-design.md:126`): ai đặt? Owner tích ô thì là biểu mẫu;
  agent đặt thì là phán đoán (hợp lệ) và phải nói vậy.
- Chọn pack theo cell (`remit-design.md:139`) và "đổi pack là Change của owner" (`:195`): hai đường cho một việc. Chọn một: cờ của
  cell (vận hành) hoặc `ChangeKind` mới (owner nói với Assistant).
- Màn `Drills`, `Releases`, `Signatures`: xem được, không sửa. Nút chạy diễn tập là rail dừng (được phép). Chữ ký phải hiện ở nơi owner
  tìm agent hỏi nó (`AGENTS.md:46-47`): một item inbox của chain đó, không phải tab riêng đếm thứ đã đếm ở Work.
- Thiết kế không đặt trạng thái nào vào browser. Server-driven giữ được, với điều kiện mỗi thực thể mới đi đủ đường store → thông báo
  → `define.ts` → `make check-gen` như `Feedback` (`web/src/rpc/define.ts:1357,1874,1909`), và mỗi RPC thay đổi lấy `Request` đầu
  tiên (`TestEveryCallThatChangesAnythingTakesARequest`, `fitness/rpc_test.go:73`) qua `act`/`once`
  (`TestEveryActionOfTheOrchestratorChecksWhoAsks`, `fitness/client_test.go:99`).
- Gói mới (`tenant`, `judge`, `policy`) cần mục trong `allowedImports` kèm lý do (`fitness/imports_test.go`); `policy` thuần thì vào
  danh sách lõi thuần. Mỗi cờ mới phải có trong README (`TestTheDocsNameEveryFlagAsItIs`). `AGENTS.md` không được dài thêm (`TestAgentsMdDoesNotGrow`).

## 7. Mười thay đổi có giá trị nhất (văn bản thay thế)

**1. Thêm vào đầu mục 2, câu sau `remit-design.md:52`:**
> "ADR đầu tiên của remit thay ADR-0003 (điều kiện của nó dựa vào ADR-0002, đã bị ADR-0010 thay, nên không bao giờ đến): một thực
> thể mới được giữ bởi quy tắc *Follow the pattern for that kind of thing*, không bởi thời gian chờ. Mỗi ADR sau ghi `Supersedes`/`Narrows`
> theo bảng ở `design-review-02-architecture.md` mục 1, và chỉ ghi `Enforced by:` các test có trong cùng thay đổi. Không văn bản nào
> trong `docs/*.md` trỏ tới một số ADR chưa có file (`TestEveryADRLinkResolves`)."

**2. Thay bảng mục 6 (thứ tự):**
> "U0 báo cho phiên ADR-0024 (`0024:88-90`), không thuộc remit. U1 Khôi phục: `store.Restore`, sao lưu ngoài người dùng và ngoài đĩa,
> diễn tập khôi phục (—). U2 Dừng: mức cell (một bản ghi), đóng mọi phiên của phạm vi, pidns, OS user theo cell, proxy phiên có
> relay cho MCP loopback và nhà cung cấp model, `Drill` (U1). U3 Cell: bộ điều khiển, cờ theo cell, bỏ mặc định một công ty (U1, U2).
> U4 Cổng v2 và sổ gọi (U2). U5 Connector khai báo (U4). U6 Rater (U4, U5 cho credential). U7 `revert_change`, bản dựng trên bản ghi
> khởi phiên (—). U8 Màn owner (U2, U4, U6, U7). Hoãn có ghi lý do: policy pack, tập niêm, `runs`, `probes`, worker đa nhà cung cấp."

**3. Thay "Action mới" ở K2 (`remit-design.md:72`):**
> "`grant.Effect` thành thang có thứ tự `Read < Reversible < Compensable < Irreversible` (H0–H2 của blueprint). Một grant hiệu ứng
> cao phủ mọi hiệu ứng thấp hơn, như hôm nay (`grant.go:10-16`); `NeedsOwner` giữ nguyên (`> Read`). Mỗi `source.Tool` khai
> `Effect` và `Data` (lớp dữ liệu) ngay trong công cụ (`source.go:77-81`): không có bảng của nền tảng riêng, không có trường `Class`.
> `source.Effect` và `grant.Effect` hợp thành một kiểu."

**4. Thay "Refusal đóng" (`remit-design.md:73-74`) và phác thảo mục 4:**
> "`type Refusal uint8`: `NoGrant`, `AboveGrant`, `Protected`, `UnknownForm`, `Stopped`, `Law`; zero = không từ chối. `Decision{Verdict,
> Refusal, Reason}`: `Verdict` nói xử thế nào (Allow, Deny, Ask), `Refusal` nói vì sao, `Reason` giữ câu agent đọc. Không có
> `Refusal.Ask` (đã là `Verdict`) và không có `Record`. Không ghi được sổ là lỗi của vỏ: `pass` ghi trước rồi mới `act`, và một lần
> ghi hỏng kết thúc bằng `OutcomeError`."

**5. Thay `func Decide(grants, stops, law, a)` (`remit-design.md:174`):**
> "`grant.Decide(grants []Grant, a Action) Decision` giữ nguyên chữ ký, và `internal/grant` vẫn không import gì. Dừng và luật được
> ghép trong lõi thuần `internal/orch/grant_access.go` (`decideAs`), nơi đã ghép capability và kết nối protected. `grantPolicy.Decide`
> (`orch/grants.go:129`) nạp `StopView` và `LawView` rồi gọi nó. Một nơi gọi prod đổi, thay vì ba nơi cộng API của `grant`."

**6. Thay K3 "Mức dừng" (`remit-design.md:88`) và xoá `stops` khỏi mục 3:**
> "Không có bảng `stops`. Trạng thái dừng ở lại với chủ hiện có: `ChainPaused` (chỉ `step` đặt), `CardStopped`, `Paused` của
> subscription, event signal và client key, `outages` của provider (`orch/health.go:39-52`). Thêm một chủ mới duy nhất: cell tạm
> dừng, một bản ghi do owner hoặc `operator` đặt. `StopView` là hàm thuần dẫn xuất từ các chủ đó; cổng và vòng lập lịch cùng đọc nó."

**7. Thay "Sổ nền tảng" (`remit-design.md:78-80`):**
> "Bảng `source_calls(id, chain_id, at, doc)` theo mẫu bảng có `doc` của store (`migrations.go:347`), `doc` là `source.Call` cộng
> `Refusal`, `params_digest` và `argv` của `run`. Nó thay `callLog` (`orch/source_calls.go`) trong cùng thay đổi. Không cột `cell`
> (một cell một DB). Không chuỗi băm ở đơn vị này: phiên đã không đọc được DB (ADR-0018); chuỗi băm đến cùng việc xuất sổ cho
> kiểm toán, và quyết cùng giữ lại (K10). Chỉ giấu `agents.log` và `remit.log` khỏi phiên nếu brief `self-development`
> (`orch/playbooks.go:72`) đổi trong cùng thay đổi. Không giấu cả `-data`, nơi có worktree."

**8. Thay "Bên trong cell" và "Cấu hình theo cell" ở K1 (`remit-design.md:62-65`):**
> "Không có bảng `settings`: cấu hình của một cell là cờ trong đơn vị launchd/systemd mà `tenant create` sinh; mặc định `-principles`,
> `-vault` (`main.go:92,100`) và JQL `DEMO` (`orch/jira.go:21`) thành rỗng. Không có bảng `members`: `-google-emails` (`main.go:123`)
> thành `-members email=role,…`. Vai trò được kiểm ở một chỗ, hàm `allowed` của `once` (`orch/request.go:47-59`) theo `method`, và
> chín phép so `Kind == ActorOwner` dùng chỗ đó. Trước khi có hơn một người: Assistant là của từng thành viên (một card mỗi người),
> và `ForOwner()` thay bằng vai trò của người mà Assistant đang nói thay."

**9. Thay "Chữ ký" ở K9 (`remit-design.md:132`) và xoá `signatures` khỏi mục 3:**
> "Không có thực thể `Signature`. Một chữ ký là câu trả lời Apply của owner hoặc `officer` cho một item `change` (`InboxChange`),
> đã được ghi bằng `Change.ApprovedBy` (`model/change.go:52-54`) và `By` của sự kiện. Item mang băm câu được ký và id sự kiện của tin
> nhắn đã xác thực cuối, do máy chủ đặt. Nó hiện trong Activity của agent hỏi, không trên một tab riêng."

**10. Thay K8 (`remit-design.md:125`) và xoá `changes (cột mới)` khỏi mục 3:**
> "Bản dựng (`Build`, để không trùng playbook `release`, `orch/playbooks.go:90`) là `vcs.revision` mà `/healthz` đã báo
> (`cmd/remit/health.go:30-41`), ghi vào bản ghi khởi phiên của mỗi card; prompt trong code đi theo bản dựng nên không cần băm
> riêng. `Change` không có `Hash` (dẫn xuất từ `After`). Tool `revert_change` ghi `Before` qua đúng đường của bản ghi đó, nên một bản
> ghi chạm rail vẫn chờ owner (`holdPlaybook`, `orch/change_item.go:42`); mô tả `read_change` bỏ hướng dẫn viết tay."

Các thay đổi nhỏ hơn: bỏ dạng stdio MCP khỏi K5 (ADR-0018 *Forms*); đặt tên `internal/rate`/`Rater` cho judge; thay `Params[].origin`
bằng vết theo card; viết K6 thành mục riêng hoặc gộp hẳn vào K5; sửa trích dẫn `kpi.go:93` → `kpi.go:106` và `env.go:79` (không có
trong `internal/orch`; `internal/source/env.go:79` là danh sách file của vault, không phải danh sách tên nhà cung cấp).

## 8. Điều thiết kế làm đúng

- Chọn cell theo tiến trình, lý do lấy từ code: `-data` đã chứa DB, `secret.key`, `owner.key`, thư mục làm việc (`main.go:90`), và một
  lọc `org_id` sai là rò dữ liệu. Đúng lập trường "chặn tổn thất ở ranh giới".
- Giữ `grant.Decide` thuần, không trạng thái ẩn, và đặt K2 cùng K3 là rail cần threat model (`AGENTS.md` *Evidence*).
- Nhận ra `decidePermission` (`orch/policy.go:188`) là tripwire, không phải ranh giới, đúng như ADR-0004 nói từ đầu.
- Tách judge (đơn lượt, không công cụ) khỏi worker để gỡ ràng buộc của ADR-0006 ở đúng chỗ nó gây hại, và hoãn worker.
- Mọi cấu hình owner đi qua change item và Assistant, credential qua trường chỉ ghi: đúng *Agent-centric*. Thành viên là rail xác thực,
  làm ở máy chủ, không qua agent.
- Nói thẳng giới hạn: egress không thấy nội dung CONNECT (`egress.go:9-11`), tác động không đảo ngược không có cận, pack chưa đối chiếu
  Công báo; không ước lượng thời gian khi không có dữ liệu.
- Coi U0 là điều kiện tiên quyết thay vì dựng màn trên một build đỏ.
- Trích dẫn đúng ở những chỗ tôi kiểm: `goal.go:28`, `gate.go:157`, `standing.go:181`, `policy.go:188`, `sandbox_darwin.go:143`,
  `plan_linux.go:38`, `library/access.go:120`, 52 migration.

## 9. Phạm vi của phản biện này

- Đã đọc: toàn bộ `remit-design.md`; `AGENTS.md:1-170`; ADR-0003, 0004, 0006, 0010, 0015 (Decision), 0017, 0018, 0021, 0022 và
  dòng trạng thái và `Enforced by` của cả 24 ADR; `fitness/adr_test.go` cả file, đầu của 10 file fitness khác; `grant.go`,
  `source/gate.go`, `orch/grants.go:1-200`, `orch/source_calls.go`, `model/actor.go`, `model/change.go`, các đoạn trích ở trên.
- Chưa đọc: ADR-0001, 0005, 0007–0009, 0011–0014, 0016, 0019, 0020, 0023, 0024 ngoài dòng trạng thái và các đoạn đã trích; `docs/design.md`
  ngoài các mục đã trích; web ngoài `define.ts` và `router.ts`. Không chạy `vue-tsc` (đỏ, đã biết). Chưa kiểm các khẳng định EXP
  của thiết kế ngoài 13 trích dẫn đã kiểm ở trên.
- Đếm nơi gọi theo mẫu `rg` dạng văn bản: có thể đếm cả comment (MEDIUM). Con số "khoảng 100 chỗ cho U2" là tổng các dòng ở mục 3 (MEDIUM).
