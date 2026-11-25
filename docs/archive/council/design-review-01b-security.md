# Phản biện bảo mật cho `remit-design.md` (lần 01b) — phần phòng thủ

> Người phản biện: kiến trúc sư bảo mật phòng thủ, có đọc code. Ngày 2026-11-01. Nối tiếp [`design-review-01-security.md`](design-review-01-security.md)
> (mục 1, bảng 37 khẳng định đã xong; thiết kế đã sửa theo). Bản này viết tiếp mục 2–7 cộng **extension host** (hướng mới của phiên: owner tự viết
> extension — forge, nguồn sự kiện, tool source, result kind — chạy ngoài tiến trình trong jail qua cổng; nền tảng không kiểm được extension làm gì thật).
> Đây là review phòng thủ: mô tả điểm yếu ở mức thiết kế và ranh giới tin cậy, kèm biện pháp; **không** viết bước khai thác, lệnh hay payload.
> Nhãn: **HIGH** = đọc đúng dòng code; **MEDIUM** = suy từ code/ADR; **LOW** = giả thuyết kèm phép thử bác bỏ.
> Lớp tấn công: **A1** nội dung ngoài lái agent; **A2** agent đã bị lái, có Bash (mặc định hôm nay); **A3** cell khác cùng host; **A4** thành viên vai thấp; **A5** người soạn/đầu độc manifest/pack/extension.

## 2. Kịch bản thiết kế bỏ sót (ngoài bảng STRIDE mục 5)

### K1 — Tenant là cell, `members`, bộ điều khiển

| # | Mối đe doạ | Lớp / ranh giới | Code/thiết kế | Khả thi | Tầm hại (no-limits) |
|---|---|---|---|---|---|
| K1-a | Vai trò nằm trên `model.Actor.Role` do xác thực đặt, nhưng `ForOwner()` hôm nay trả true cho cả `ActorAssistant` (`actor.go:37`, HIGH). Thêm `Role` mà giữ `ForOwner` theo Kind thì một `operator` hay một Assistant bị lái vẫn "là owner" ở mọi chỗ đọc `ForOwner` (reopen chain, tin đọc trước). | A4/A2 → rail owner | `model/actor.go:37`; bảng hành động→vai trò (K1) là **nơi thứ hai** định nghĩa quyền | MEDIUM: leo quyền do hai chủ của một khái niệm | Rộng: một vai thấp chạm việc của owner |
| K1-b | `members(id, who, roles)` là bảng trong **database của cell**; database nằm trong `Hide` của phiên nhưng **không** của cell khác cùng host nếu chung OS user (K1 Hệ quả thừa nhận, `main.go:92` HIGH). A3 đọc/sửa `members` của cell láng giềng. | A3 → ranh giới cell↔cell | `cmd/remit/agents.go:88-100`: `Hide` chỉ che tệp của **chính** phiên, không có khái niệm "của cell khác" | MEDIUM nếu chung user; HIGH trước khi có OS user mỗi cell | Rộng: thêm thành viên vai `owner` cho cell khác |
| K1-c | Bộ định tuyến "mỏng theo tên miền" (K1) là **cửa vào mới** đứng trước mọi cell; nó quyết định request của tên miền nào tới cell nào. Một lỗi ánh xạ gửi phiên owner của công ty A tới cell B. | A1/A4 → ranh giới control-plane | K1 "một bộ định tuyến mỏng… chỉ chuyển HTTP theo tên miền" (chưa có code) | LOW (chưa xây): phép thử — hai cell, xác thực chéo bị từ chối | Rộng, rò dữ liệu xuyên công ty |

### K2 — Cổng v2, `gate_calls` băm chuỗi, egress proxy

| # | Mối đe doạ | Lớp / ranh giới | Code/thiết kế | Khả thi | Tầm hại |
|---|---|---|---|---|---|
| K2-a | Egress proxy "cho mọi đích, ghi và đếm" **không thấy nội dung đường hầm CONNECT** (`egress/egress.go:9-15`, HIGH). Nó chặn *đích*, không chặn *điều nói với đích đã cho phép*: một H2 (email, chuyển tiền) tới một host hợp lệ đi lọt, cổng chỉ ghi "đã nối tới host X". | A2 → ranh giới agent→ngoài | `egress.go:9-15`; thiết kế K3 thừa nhận | HIGH: bản chất của CONNECT proxy | Vô cận (H2): confused deputy qua một đích hợp lệ |
| K2-b | `grant.Decide` thuần nhận `Action{Source,Connection,Effect}` (`grant.go:49-52,73`, HIGH); `Class/Data/Params` của thiết kế do **bảng nền tảng** gán ở `decideAs` tại orch (`grants.go:73-86`, HIGH). Nếu extension/manifest tự khai `class` (K5), một endpoint ghi khai là `read`/H0 thì cổng cho qua không hỏi owner. | A5 → ranh giới manifest→cổng | `grant.go`; `orch/grants.go:73-148`; K2 "Class do bảng nền tảng đặt" (đúng hướng, chưa có bảng) | MEDIUM: đúng nếu bảng lớp để manifest khai | Vô cận: ghi/H2 trốn Ask |
| K2-c | `gate_calls` băm chuỗi `hash=H(prev_hash‖row)` do máy chủ ghi. Nhưng `work/`, `claude/`, `library/` của phiên nằm **trong** thư mục dữ liệu, không ẩn được (`agents.go:86`, review-01 #33, HIGH). Băm chuỗi phát hiện cắt xén chỉ khi **neo** nằm ngoài tầm phiên; nếu neo ghi trễ, một cửa sổ giữa hành động và neo không bảo vệ được. | A2 → ranh giới bản ghi nền tảng | `cmd/remit/agents.go:85-98`; K2 "neo băm ra ngoài đĩa một dòng mỗi ngày" | MEDIUM: cửa sổ = chu kỳ neo | Repudiation trong cửa sổ neo |
| K2-d | Cổng thứ hai `decidePermission` (`policy.go:188`, regex, HIGH) hạ thành tripwire. Nếu bỏ Bash khỏi mặc định (U2) nhưng một playbook khai lại Bash qua capability, tripwire regex không phải lớp tính được: nó bỏ sót lệnh ngoài mẫu. | A2 → ranh giới agent→host | `orch/policy.go:188-223` | HIGH nếu Bash còn đường quay lại | Rộng (dưới user máy chủ) |

### K3 + extension host — Dừng, egress, hai OS user; extension chạy trong jail qua cổng

| # | Mối đe doạ | Lớp / ranh giới | Code/thiết kế | Khả thi | Tầm hại |
|---|---|---|---|---|---|
| K3-a | `stops(scope,id,by,at)` là nguồn sự thật `Decide` đọc (`Stopped`). `Decide` thuần đọc một `StopView` truyền vào; nếu `StopView` được tải một lần đầu request rồi cache, một dừng giữa lời gọi dài không có hiệu lực tới byte cuối. Blueprint I5 đòi đo ở **mạng host**, không ở bảng. | A2 → rail dừng | thiết kế K3; blueprint I5 (`:234`) | MEDIUM | Dừng trễ = cửa sổ tổn thất H2 |
| K3-b | **Extension là program T2**: credential trong env của jail, egress = allowlist của connection (`program.go:55-61`, HIGH; ADR-0018:60). Egress chỉ chặn *đích*; extension đọc được credential trong env của chính nó và gửi nó tới một đích **đã cho phép** (ADR-0018:94 "accepted residual"). `RedactText` bắt *hình dạng* token, không bắt giá trị đã mã hoá (`source/redact.go:11`, HIGH). | A5 → ranh giới extension→ngoài | `source/program.go:55-61`; `egress/egress.go:9-15`; ADR-0018:94 | HIGH: residual đã ghi nhận | Rò credential của connection đó |
| K3-c | macOS: relay của jail chương trình nghe trên **loopback của host** `127.0.0.1:0` (`sandbox_darwin.go:82`, HIGH; ADR-0021:64): tiến trình khác của máy (A3, hay một extension khác) nối tới cổng đó và với tới cái cổng cho phép, tới khi program kết thúc. Không có loopback riêng cho program trên macOS. | A3 → ranh giới jail (macOS) | `jail/sandbox_darwin.go:82-93`; ADR-0021:64 | HIGH trên macOS, không trên Linux (NEWNET) | Mượn egress của extension khác |
| K3-d | Extension viết sai/độc **vẫn chạy dưới OS user của phiên** cho tới khi có OS user riêng cho phiên (K3 (ii), ADR-0004 chưa đóng). `setsid` con sống quá deadline trên macOS (không pidns, ADR-0021:63). Dừng theo bảng `stops` không giết được con đã tách. | A5/A2 → rail dừng + cell↔host | ADR-0021:63; `plan_linux.go:37-40` (pidns chỉ khi Jail≠nil) | HIGH trên macOS | Tiến trình nền sống sót lệnh dừng |

### K7 — Lớp xác minh, tập niêm, replay

| # | Mối đe doạ | Lớp / ranh giới | Code/thiết kế | Khả thi | Tầm hại |
|---|---|---|---|---|---|
| K7-a | **Tập niêm** (anchors, probes, canary, ngưỡng) lưu ở "đường dẫn trong `Hide` của mọi phiên, không công cụ nào đọc". Nhưng nó nằm trong database/thư mục dữ liệu cùng cell; một agent có Bash (A2) dưới user máy chủ đọc database bằng `secret.key` (secrets.go:8, HIGH) — cùng con đường ADR-0009:48 nêu cho client key. Biết vị trí canary và nhãn lỗi gieo thì agent đối kháng qua được kiểm (blueprint B6, 2606.05647). | A2 → tập niêm | `secrets/secrets.go:8`; ADR-0009:48; blueprint `:451` | MEDIUM (tới khi OS user riêng); HIGH trước đó | Vô hiệu hoá lớp xác minh |
| K7-b | **Replay (`runs`, pass^k)** chạy lại tác vụ ghi lại. Nếu tác vụ ghi lại gọi connection thật (không phải bản ghi tĩnh), replay **tự nó là hiệu ứng ngoài** lặp K lần — một H1/H2 nhân K. Thiết kế nói "không phải thí nghiệm trên khách" nhưng không chỉ ra cơ chế chặn `runs` chạm connection sống. | A5 (tác giả playbook replay) → ngoài | K7 "Phát lại (pass^k)"; blueprint 3.5 `:316` | MEDIUM | Nhân hiệu ứng H2 |
| K7-c | `model.Finding.Reason` thành tập đóng + `EvidenceRef` do nền tảng cấp. Nếu `EvidenceRef` là id do agent chuyền (không phải nền tảng tự ghi rồi tham chiếu), một agent chèn id trỏ tới bằng chứng của finding khác. Blueprint I6 đòi evidence_ref **do nền tảng cấp**. | A2 → ranh giới verdict | thiết kế K7; blueprint I6 (`:235`) | LOW: phụ thuộc hiện thực | Ngụy tạo bằng chứng |

### K9 — Bề mặt owner, chữ ký, hàng đợi

| # | Mối đe doạ | Lớp / ranh giới | Code/thiết kế | Khả thi | Tầm hại |
|---|---|---|---|---|---|
| K9-a | **Ngập hàng đợi ngoại lệ** (blueprint B6, [C02] xếp đây trước): A1/A2 sinh nhiều mục H2 nhỏ để owner mỏi mà bấm "Apply". Thiết kế gom trùng + xếp theo lớp nhưng không có cận tốc độ sinh mục cho một chain/agent. | A2 → owner | K9 "Loại trùng"; blueprint 3.10 `:384` | HIGH: người duyệt kém khi khối lượng lớn (E21) | Owner duyệt nhầm một H2 |
| K9-b | **Chữ ký gắn tin nhắn xác thực cuối của owner** do máy chủ làm (`Signature`, K9). Rủi ro: agent soạn một **câu hỏi** khiến owner trả "ừ, ký đi"; câu đó thành tin xác thực mà chữ ký trỏ tới (blueprint B6 "agent soạn câu hỏi để owner nói merge"). Máy chủ gắn đúng tin, nhưng *nội dung* tin là do agent dẫn dắt. | A1/A2 → rail chữ ký | K9 "Máy chủ gắn chữ ký với tin nhắn đã xác thực cuối" | MEDIUM | Chữ ký pháp lý cho việc agent muốn |
| K9-c | `mark_live` của Assistant (`assistant_tools.go:255`, HIGH) "merge nothing" nhưng mở đường cho chain xác minh và tiếp tục. Một Assistant bị lái (A2, `ForOwner()`=true) gọi `mark_live` sai, chain coi hiệu ứng đã live và đi tiếp. | A2 → rail | `orch/assistant_tools.go:255`; `actor.go:37` | MEDIUM | Chain hành động trên trạng thái sai |

### K10 — Luật là dữ liệu, policy pack

| # | Mối đe doạ | Lớp / ranh giới | Code/thiết kế | Khả thi | Tầm hại |
|---|---|---|---|---|---|
| K10-a | **Pack do luật sư ký, đổi pack là Change của owner.** Nhưng chữ ký pack cùng cơ chế `Signature` của K9: nếu chữ ký chỉ là "tin owner xác thực" chứ không ký **nội dung pack** (băm pack), một pack bị thay sau khi ký mà chữ ký vẫn hợp lệ. | A5 → F3 luật | K10; K9 `Signature`; blueprint I12 (`:241`) đòi băm | MEDIUM | Luật sai được ép như luật đúng |
| K10-b | `Law` là hàm thuần `(Action, pack)`. Nhưng tính hợp pháp của **nội dung** (phỉ báng, lừa dối) không quyết bằng hàm thuần (blueprint 3.2 `:258`): pack ép luồng dữ liệu (I17, I18), không ép nội dung. Thiết kế đúng ở điểm này nhưng không nói rõ giới hạn trong K10 — người đọc tưởng `Law` phủ mọi vi phạm luật. | — → rủi ro còn lại | blueprint `:481` rủi ro còn lại | HIGH (đã biết) | H2 nội dung trái luật vẫn ra |
| K10-c | `events` "never deleted" (`store/pack.go`, review-01 #15 HIGH) đối nghịch lớp giữ lại/xoá của K10. Hai cơ chế ghi/xoá trên cùng bảng là nguồn mâu thuẫn (một chủ cho "sự kiện không xoá", một cho "xoá theo lớp"). | A4 (yêu cầu xoá GDPR) → luật | K10 "Giữ lại và xoá"; `store/pack.go:13-17` | MEDIUM | Không xoá được dữ liệu cá nhân đúng hạn |

## 3. Bề mặt tấn công **do chính thiết kế tạo ra**, và biện pháp tối thiểu (không giới hạn sáng kiến agent)

- **Egress proxy — điểm đơn lẻ.** Mọi lưu lượng phiên qua một proxy: nó sập thì cả cell mất mạng; nó bị chiếm thì thấy mọi đích. *Biện pháp (không giới hạn):* proxy là tiến trình riêng, không giữ credential (đã đúng, `egress.go:9`), cấu hình cố định khi chạy (`Config` bất biến, `egress.go:60-75`, HIGH); giám sát sống/chết là một `Drill`. Không thêm allowlist (sẽ là giới hạn).
- **Neo băm (`gate_calls`).** Neo ngoài đĩa là đích ghi mới → một kho ngoài. *Biện pháp:* neo chỉ-thêm, một chiều (append-only, máy chủ ghi), và owner đọc chênh lệch băm trong bảng điểm; không cho agent nào tham chiếu kho neo.
- **Vị trí tập niêm.** Tạo một "bí mật thứ hai cạnh credential" (R2) → nếu lộ thì vô hiệu toàn bộ xác minh. *Biện pháp tối thiểu:* tập niêm **không nằm trong database của cell** (khác credential, vốn chỉ cần kín với DB); để ngoài tầm cả agent lẫn job sống, chỉ job đo đọc, dưới OS user khác phiên (phụ thuộc K3 (ii)).
- **Bảng `members`.** Dữ liệu quyết định quyền → đích leo quyền. *Biện pháp:* một bảng duy nhất hành động→vai trò (một chủ), `Decide`-style thuần đọc nó; mọi ghi `members` là rail owner ở máy chủ (K9), ghi ai làm.
- **Manifest/extension.** Bề mặt B3 (đầu độc mô tả công cụ, rug-pull). *Biện pháp:* ghim **băm nội dung** của manifest/extension ở lần owner Apply (SC12, blueprint `:475`); đổi băm = Change mới của owner; mô tả công cụ trung thực do nền tảng gán `class`, không do manifest tự khai (xem 2-K2-b).
- **Control plane / bộ định tuyến.** Cửa vào mới trước mọi cell. *Biện pháp:* bộ định tuyến **không đọc dữ liệu cell**, chỉ chuyển theo tên; xác thực làm **trong** cell, không ở router.
- **Bảng `stops`.** Nếu `Decide` đọc cache cũ, dừng không hiệu lực. *Biện pháp:* đo dừng ở **mạng host và bảng tiến trình** (blueprint I5, SC11), không ở bảng `stops`; `Drill` đo byte cuối.

## 4. Ba điểm cố định trên Linux và macOS

Ba điểm cố định (quyết định của owner): **(1) dừng tay của owner, (2) credential không bao giờ vào ngữ cảnh agent, (3) tuân luật.**

| Điểm | Linux | macOS |
|---|---|---|
| (1) Dừng tay | **Gần đạt với K3.** `CLONE_NEWPID` cho jail giết cả cây (`plan_linux.go:39`, HIGH). Còn sai: phiên (session, không phải program) hôm nay **không** có pidns (chỉ khi `Jail≠nil`, `plan_linux.go:37-40`); `setsid` của phiên sống sót tới khi có pidns cho phiên hoặc OS user riêng để `kill` theo user. | **Chưa đạt.** Không có pidns: `setsid` con sống quá deadline (ADR-0021:63, HIGH). Dừng thật cần **OS user riêng cho phiên** rồi `kill` theo user. Đó là việc host, chưa có. |
| (2) Credential kín | **F2 vẫn bị vi phạm cho tới khi đóng lỗ.** Đúng cho credential nguồn (T0/T1, ADR-0017). Sai cho: tệp đăng nhập model là credential phiên giữ (`childenv.go:29-32`, HIGH); `~/.ssh`, glab đọc được (ADR-0018:112-114); T2 rò giá trị đã mã hoá (ADR-0018:94). Phiên cùng user máy chủ đọc database + `secret.key` (secrets.go:8, HIGH) → mọi secret sealed. Đóng bằng OS user riêng + GitLab connection kind. | **Sai thêm hai chỗ.** `kern.procargs2`: phiên đọc env của mọi tiến trình cùng user (ADR-0021:66) → env server phải không chứa secret. Token MCP của phiên khác: header `--mcp-config` qua `${NAME}` trong env (options.go:62-93, HIGH), mà env đọc được qua procargs2. Cùng user → cùng lỗ. |
| (3) Tuân luật | **Khả thi theo cấu trúc** nếu `Law` trong cổng (K10) và cổng phủ mọi đường ra (U2). Còn sai: hôm nay chưa có pack, `events` không xoá được (2-K10-c), lời gọi model mang dữ liệu khách đi qua kênh phiên ngoài cổng (blueprint B5, ADR-0018:71-72). | **Như Linux, cộng B5 rõ hơn:** `sandbox_darwin.go:143` phiên `(allow default)` nên mạng ra ngoài cổng; dữ liệu cá nhân tới nhà cung cấp model không qua `LegalPolicy`. Phải chuyển phiên sang `(deny default)` chỉ-proxy (mẫu program đã có ở `:125`, HIGH). |

**Kết luận mục 4 (MEDIUM):** cả ba chỉ đạt **sau** khi có OS user riêng cho phiên (K3 (ii)) trên cả hai OS; macOS cần thêm chuyển phiên vào jail chỉ-proxy. Không OS nào đạt đủ hôm nay.

## 5. Hai OS user (user của cell, user của phiên) cần gì thêm

- **Sở hữu tệp.** Thư mục dữ liệu, `work/`, `claude/`, `library/` do user cell sở hữu; phiên (user khác) cần **đọc/ghi `work/` và `claude/`** nhưng **không** đọc `secret.key`, database, `owner.key`, env (quyền nhóm/ACL, không chỉ `Hide` của Seatbelt/namespace). Hôm nay `Hide` đóng vai này cho cùng-uid; user riêng làm nó bằng quyền POSIX.
- **Keychain (macOS).** Đăng nhập Claude Code của agent nằm trong Keychain dưới tên config dir (`agents_darwin.go:13-16`, HIGH). User phiên khác cần keychain login **riêng** của nó, nếu không `claude auth status` fail. Đây là chi phí vận hành mới.
- **launchd user domain (macOS) / systemd (Linux).** Server chạy qua launchd ở user domain của owner (`remit.plist`, memory). Phiên dưới user khác cần hoặc một launchd/`launchctl asuser` cho user đó, hoặc server (chạy như root hoặc có quyền) `setuid` xuống user phiên khi start phiên. Hôm nay `helperAttr` ánh xạ **chính uid của server** (`namespaces_linux.go:76`, HIGH) — tức *chưa* đổi user; đổi user cần quyền server không có khi chạy không-root.
- **Cache dựng chung.** `GOCACHE`, `GOMODCACHE`, `GOPATH` kế thừa từ env server (`childenv.go:52-53`, HIGH) và dùng chung cho check của submit + shell phiên. User riêng cho phiên thì cache này phải **đọc được bởi cả hai user** (nhóm chung) hoặc mỗi user một cache (chậm hơn, không giới hạn — chấp nhận được theo lập trường "chất lượng trên chi phí").
- **Tệp đăng nhập model.** Credential duy nhất phiên giữ (`childenv.go:29-32`, HIGH). Với user phiên riêng, tệp này phải do **user phiên** sở hữu, không phải user cell/server; nếu không phiên không đọc được login của nó. Trên macOS là mục Keychain của user phiên.

## 6. Mô hình supply-chain cho extension

Owner viết extension (forge, nguồn sự kiện, tool source, result kind), agent của owner soạn, owner Apply. Nền tảng **không kiểm được extension làm gì thật** (B3: tạo/dùng công cụ có lỗ hổng >76%, không nhận ra công cụ độc ~93%, 2509.26354).

**Cận thực sự giữ (enforced, không phải trust):**
- **Jail (Linux).** User+mount+net+pid namespace, không mạng trừ socket proxy, root chỉ-đọc trừ `work/`, drop mọi capability trước exec (`supervise_linux.go:24-45`, HIGH; ADR-0018:67). Một extension là program T2 bị giam thật trên Linux.
- **Egress allowlist theo đích.** Extension chỉ nối tới `Egress` của connection (`program.go:59-61`, HIGH); DNS rebinding/SSRF bị `routable()` chặn (dải riêng/loopback/link-local/NAT64, `egress/policy.go:109-152`, HIGH). **Chỉ đích**, không nội dung.
- **Content hash pinning.** Băm manifest/extension ở Apply (SC12) phát hiện thay đổi sau đó — nếu hiện thực đúng (chưa có code).
- **Credential scope.** Extension nhận credential read-only của owner; `write` là Change của owner (`grant.go:95`, HIGH).
- **Không chạm data dir của cell.** Jail chỉ thấy `work/` + system paths (`sandbox_darwin.go:127-133`, HIGH; mounts_linux read-only).

**Chỉ là trust (không ép được):**
- Extension **đọc được credential trong env của chính nó** và gửi tới một đích **đã cho phép** (ADR-0018:94, residual đã nhận). `RedactText` bắt hình dạng token, không bắt giá trị mã hoá (`redact.go:11`, HIGH).
- Extension tự khai `class`/`effect` nếu bảng lớp để nó khai (2-K2-b): nền tảng phải là chủ của bảng lớp, không để manifest khai.
- macOS: không mem limit, CPU limit bị Go bỏ qua, `setsid` sống quá deadline, relay trên loopback host (ADR-0021:60-64, HIGH). Trên macOS jail **yếu hơn** Linux đúng các điểm này.
- Extension **đúng chức năng** (một forge adapter không âm thầm gửi data đi nơi khác): không kiểm được; chỉ cận bằng egress + credential scope + jail.

**Tóm:** blast radius của một extension độc = {dữ liệu trong `work/` của chain đó} ∪ {credential của connection nó được cấp} ∪ {các đích trong egress allowlist của nó}. Giữ được trên Linux với jail đầy đủ; **yếu trên macOS** (các residual trên) và **yếu khi phiên chung user** với server.

## 7. Mười thay đổi giá trị nhất cho thiết kế (xếp hạng, kèm văn thay thế)

1. **(K3/ADR-0004) Nói rõ OS user riêng cho phiên là tiên quyết của cả ba điểm cố định.** Thay câu K3 (ii) bằng: *"Mỗi phiên chạy dưới một OS user khác user của cell và của server. Ba điểm cố định (dừng tay, credential kín, tuân luật) chỉ đạt sau việc này trên cả hai OS; tới khi đó, mọi khẳng định về F1/F2 là MEDIUM."*
2. **(K2) Lớp hiệu ứng do nền tảng gán, không do manifest/extension khai.** Thêm vào K2: *"`Class` và `effect` của một endpoint do một bảng của nền tảng (connection kind × operation) quyết; một thao tác chưa có trong bảng là H2 và `write` cho tới khi owner thêm nó. Manifest/extension đề nghị, nền tảng gán."* (blueprint 3.2 `:260`, I1.)
3. **(K7/K9/K10) Chữ ký ký băm nội dung, không chỉ tin xác thực.** Thay mô tả `Signature`: *"Chữ ký lưu băm của **đối tượng được ký** (pack, release, hồ sơ) cùng id tin nhắn xác thực của owner; đổi nội dung sau khi ký làm chữ ký không khớp."* (blueprint I12 `:241`.)
4. **(K7) Tập niêm ra ngoài database của cell.** Thay R2: *"Tập niêm (anchors, probes, canary, ngưỡng) không nằm trong database hay thư mục dữ liệu của cell; nó dưới OS user của job đo, phiên và agent không đọc được ngay cả khi đọc database bằng `secret.key`."* (secrets.go:8, ADR-0009:48.)
5. **(K2) Nói thẳng egress proxy không chặn nội dung H2.** Thêm vào bảng STRIDE mục 5 (dòng Cổng/egress): *"Proxy chặn đích, không nội dung CONNECT; một H2 tới đích hợp lệ không bị proxy chặn — chặn nội dung là việc của cổng công cụ (class/effect) và SC4 hoãn-có-huỷ, không của proxy."* (`egress.go:9-15`.)
6. **(K9) Cận tốc độ sinh mục ngoại lệ cho mỗi chain/agent.** Thêm: *"Số mục H2 một chain mở trong một cửa sổ có tripwire (SC6, chỉ báo); hàng đợi hiện tốc độ sinh, để ngập hàng đợi là một tín hiệu owner thấy, không phải thứ làm owner mỏi."* (blueprint B6, [C02].)
7. **(K7) `runs`/replay không chạm connection sống.** Thêm: *"Replay chạy trên bản ghi tĩnh của đầu vào, không gọi connection thật; một tác vụ ghi lại mà hiệu ứng ra ngoài không đảo ngược được thì không vào bộ replay."* (blueprint 3.5 `:316`.)
8. **(K1) Một chủ cho quyết định quyền; `ForOwner` không mở cho vai mới.** Thêm: *"Bảng hành động→vai trò là chủ duy nhất của quyền; `ForOwner()` giữ nghĩa cũ (owner + Assistant trên lời owner), vai `operator`/`viewer` không đi qua nó."* (`actor.go:37`.)
9. **(K10) Hợp nhất ghi/xoá `events` về một chủ.** Thay câu `events`: *"Lớp giữ lại thay 'không bao giờ xoá': `events` có `retention_class`; job xoá theo lớp là chủ duy nhất của vòng đời sự kiện; sổ bằng chứng pháp lý là một lớp giữ lâu, không phải 'không xoá'."* (`store/pack.go:13-17`.)
10. **(macOS) Chuyển phiên sang `(deny default)` chỉ-proxy.** Thêm vào K3: *"Trên macOS phiên chạy `(deny default)` + `bsd.sb` với `network-outbound` chỉ tới loopback của proxy, như jail chương trình đã làm (`sandbox_darwin.go:125-131`); `(allow default)` ở `:143` bị thay. Điều macOS vẫn kém (mem, CPU-ignore, setsid, relay loopback host) liệt kê như ADR-0021."* (`sandbox_darwin.go:125-143`, ADR-0021:60-64.)

## 8. Điều thiết kế làm đúng

- **Cổng thuần, một đường ra, tập mã từ chối đóng** đúng reference-monitor và blueprint I1; `grant.Decide` thuần không import module (`grant.go:1-8,73`, HIGH) giữ được bài kiểm thuộc tính.
- **Egress đã chống SSRF/rebinding tốt**: `routable()` chặn dải riêng/loopback/link-local/NAT64/Teredo, giải tên ở mỗi lần dial (`egress/policy.go:109-152`, HIGH) — vượt nhiều hệ thống. Nhận thẳng "không thấy nội dung CONNECT" là trung thực (`egress.go:9-15`).
- **Chữ ký do máy chủ gắn tin owner, công cụ chỉ nêu tên câu** (K9) đúng SC10, chống rail kéo theo lời bịa.
- **Thừa nhận giới hạn thật thà**: review-01 sửa 6 khẳng định sai; thiết kế giữ "tác động H2 vô cận" (mục 5 lưu ý), không giả vờ giải xong.
- **Jail Linux mạnh**: drop capability, no_new_privs, pidns+netns cho program, root chỉ-đọc (`supervise_linux.go`, `namespaces_linux.go`, HIGH) — cận extension giữ được trên Linux.
- **Connector manifest là dữ liệu qua cổng, không bỏ qua cổng** (K5 "declared không bỏ qua cổng") — extension host thừa kế đúng rail, không mở đường vòng.
- **Hai OS user tách đúng hai khoảng hở** (cell↔cell và agent↔server), không gộp — đúng bản chất ADR-0004 vs K1.

---
*Nguồn code tự đọc lại lần này (HIGH): `grant/grant.go` (cả file), `source/gate.go` (cả file), `source/program.go:17-80`, `source/connection.go:200-260`, `egress/egress.go:1-140`, `egress/policy.go` (cả file), `jail/sandbox_darwin.go:1-152`, `jail/plan_linux.go`, `jail/namespaces_linux.go`, `jail/supervise_linux.go:12-45`, `childenv/childenv.go`, `secrets/secrets.go:1-60`, `auth/local.go`, `clientmcp/handler.go`, `door/door.go:1-90`, `orch/grants.go:1-235`, `orch/policy.go:170-230`, `orch/tools.go:429-470`, `orch/assistant_tools.go:221-262`, `orch/lifecycle.go:300-345`, `agent/options.go:45-110`, `model/actor.go`, `cmd/remit/agents*.go`, `cmd/remit/main.go:195-265`. ADR 0004/0009/0017/0018/0021/0022 (cả file). Blueprint mục 3.1/3.2/3.7/3.8/5/7.2.*
