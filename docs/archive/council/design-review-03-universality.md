# Đánh giá thiết kế 03: tính phổ quát và khả năng áp dụng

> Người đánh giá: UNIVERSALITY AND ADOPTION REVIEWER, ngày 2026-11-01. Đối tượng: [`docs/remit-design.md`](../remit-design.md) (K1–K11, U0–U9).
> Câu hỏi: thiết kế có thật sự cho remit chạy ở **mọi công ty, mọi hệ thống** không, và gãy ở đâu. Phương pháp: dắt ba công ty rất khác nhau đi qua thiết kế
> từng bước, đối chiếu với code hiện có. Chỉ đánh giá, không sửa thiết kế hay code.
> Nhãn: **HIGH** = tôi đã đọc đúng đoạn code hoặc trang nguồn; **MEDIUM** = suy ra từ nguồn đã đọc; **LOW** = phỏng đoán, cần kiểm. Không phải tư vấn pháp lý.

## 0. Kết luận trước

- **Thiết kế phổ quát cho một loại công ty: công ty mà công việc chính là phần mềm trong repo GitLab/GitHub, chạy trên một host Linux/macOS, người dùng là một
  owner.** Với A (SaaS EU) nó gần đủ sau U1–U4; với C (shop Shopify Việt Nam) phần lớn việc của công ty không có đường ra; với B (ngân hàng) nó không qua được
  ngày 0 vì mạng, định danh, phê duyệt hai người và nền tảng triển khai. **HIGH** cho các điểm gãy dẫn code; **MEDIUM** cho kết luận tổng.
- **Năm điểm mở rộng là chưa đủ.** Ba adopter cùng đụng năm trục mà bảng ở `remit-design.md:15-21` không có: **forge** (`-forges` chỉ nhận `gitlab`, `github`,
  `cmd/remit/main.go:99`, `internal/forge/forge.go:22-23`), **nhà cung cấp định danh** (chỉ Google qua Firebase hoặc khoá local, `main.go:119`),
  **backend và vùng của model** (worker), **vị trí executor** (hệ thống sau VPN), và **loại kết quả** (mọi kết quả là draft MR, `docs/design.md:35-38`).
- Câu "mọi công ty bị chặn bởi (a) pack luật, (b) hệ thống cũ, (c) hiệu ứng H2" (`remit-design.md:23-24`) **đúng nhưng thiếu**: các chặn lớn nhất với B và C
  không thuộc (a)–(c) mà là sản phẩm (kết quả = MR), định danh, và nền tảng chạy (K8s).

## 1. Điều thiết kế làm đúng cho phổ quát (giữ)

- Cell một tiến trình một thư mục (K1) là đơn vị đúng: dừng, xoá, xuất theo cell là thao tác OS, không lọc `org_id` (`remit-design.md:56-59`). **HIGH** về lập luận.
- Luật là dữ liệu (K10), manifest connector là dữ liệu (K5), cổng thuần (K2, `internal/grant/grant.go:73`): đúng hướng, không nhánh theo công ty.
- Judge qua API tách khỏi harness worker (K4): giải đúng ràng buộc độc lập mà không cần harness thứ hai.
- `-language` đã có (`main.go:107`): owner Việt Nam đọc được tiếng Việt ngay ngày 0. **HIGH**.

## 2. Adopter A: SaaS B2B 40 người, EU, AWS, Postgres, Stripe, HubSpot, Zendesk, GitHub, Slack

Trạng thái: **Chạy** = thiết kế như viết đủ; **Thiếu** = cần một mảnh chưa có (tên mảnh, K/U nên phủ, nội dung bắt buộc); **Không** = không chạy được, vì sao.

| Bước | Trạng thái | Bằng chứng / mảnh thiếu |
|---|---|---|
| Cấp cell | Thiếu (U1, U9) | `remit tenant create` chưa có (K1). Mẫu systemd chưa từng chạy trên host systemd (`docs/deploy.md:87`). Mặc định `/workspace/example`, `DEMO` phải bỏ (`main.go:92,100,116`; `orch/jira.go:21`), U1 đã ghi. Repository catalog nằm trong "vault" JSON (`deploy.md` mục 2): U1 phải đưa catalog vào `settings` hoặc một thực thể, do agent ghi theo lời owner. |
| Owner, thành viên | Thiếu (K1, K9) | Đăng nhập chỉ Google/Firebase hoặc local loopback (`main.go:119`, `cmd/remit/signin.go:19-22`). Công ty dùng Microsoft 365 không đăng nhập được. Cần `-sign-in=oidc` (IdP của công ty), vai trò từ nhóm IdP. `members` của K1 không nói nguồn danh tính. |
| GitHub | Chạy | Forge GitHub có (`internal/forge/github.go:23`), nguồn `github` đọc issue (`internal/source/source.go:41`). Bảo vệ nhánh do người vận hành đặt (`design.md:295-302`). |
| Postgres | Không (qua manifest) | Postgres là giao thức dây, không REST/CLI/MCP chuẩn. Đường có sẵn: qua Bytebase (`source/bytebase.go:49`, engine POSTGRES) nếu công ty dùng Bytebase; hoặc `psql` qua `ProgramSpec` (form env), khi đó "chỉ đọc" là quyền của credential, không phải code (`source/doc.go`, mục Programs). Cần provider Go có guard SQL như `starrocks.go` (403 dòng). |
| AWS | Thiếu (K5) | CloudWatch có (`source.go:44`). RDS/ECS/S3 cần `aws` CLI (form env): ký SigV4 không biểu diễn được bằng "mẫu xác thực = tên biến credential" của manifest (`remit-design.md:108`). |
| Stripe | Thiếu (K5) | REST, form-encoded, `Idempotency-Key` cho mọi POST, khoá giữ ≥24 giờ ([Stripe](https://docs.stripe.com/api/idempotent_requests)): khớp yêu cầu `idempotency_key` của K5. Hoàn tiền là H2 (blueprint 3.8): cần hoãn-có-huỷ, mặc định tắt. Webhook Stripe ký bằng header riêng: cửa `/hooks` chỉ nhận token GitLab, bearer, `?token=` hoặc HMAC kiểu GitHub (`internal/door/door.go:116-135`) nên chỉ vào được bằng token trong URL, mất xác minh chữ ký. |
| HubSpot, Zendesk | Thiếu (K5) | REST, token tĩnh: manifest REST của K5 biểu diễn được (MEDIUM). Zendesk ghi trả lời khách là H2: không có công cụ ghi nào hôm nay ("Every tool reads and nothing else", `source/doc.go:8`). |
| Slack | Không (phần nhận) | Web API REST: manifest được. Nhận sự kiện: Socket Mode là WebSocket, không cần URL công khai ([Slack](https://docs.slack.dev/apis/events-api/using-socket-mode)); manifest không có dạng WebSocket. Events API qua HTTP cần trả lại `challenge`, còn cửa "asks nothing of the body" (`door.go:5`) và chỉ `POST` (`door.go:26`). |
| Agent làm được ngày 0 | Một phần | Sửa code, review, QC, mở draft PR trên GitHub; đọc CloudWatch, GitHub issues. Không đọc Postgres/Stripe/HubSpot/Zendesk/Slack cho tới U4; không ghi gì ra ngoài. |
| Luật (GDPR) | Thiếu (K10, U7) | Pack EU cần luật sư (thiết kế nói thẳng). Thiếu trong K10: điều 22 GDPR, quyền không bị quyết định "based solely on automated processing" có hệ quả pháp lý ([gdpr-info](https://gdpr-info.eu/art-22-gdpr/)); pack phải có lớp "quyết định ảnh hưởng đáng kể tới một người" dẫn tới `Ask`, không chỉ lớp H theo khả năng đảo ngược. |
| Dừng | Thiếu (U3) | K3 đủ cho host đơn. Đo được ở proxy, OS user mỗi cell. |
| Sao lưu | Thiếu (K11, U9) | Hôm nay bản sao chung đĩa với dữ liệu (`deploy.md:142`), không có `Restore`. K11 phủ, cần đích EU (S3 eu-*) vì backup chứa dữ liệu cá nhân. |
| Nâng cấp | Chạy một phần | `make release` giữ `previous` (`deploy.md`, Backups and upgrades); lặp qua cell là việc của `tenant upgrade` (K1). |
| Rời đi, xuất, xoá | Thiếu (U7) | Xem mục 8. |

**Ba điểm gãy đầu tiên khi dùng thật (A):**
1. **Vùng dữ liệu model.** Worker chỉ chạy Claude Code với login của máy (`deploy.md:37`); API trực tiếp của Anthropic chỉ có `inference_geo` là `global` hoặc `us`
   ([data residency](https://platform.claude.com/docs/en/manage-claude/data-residency)); đường EU là Bedrock với profile `eu.*`
   ([AWS](https://docs.aws.amazon.com/bedrock/latest/userguide/geographic-cross-region-inference.html)), mà Remit lọc hết biến môi trường khỏi phiên
   (`internal/childenv/childenv.go:33-61`, không có `CLAUDE_CODE_USE_BEDROCK` hay `AWS_*`). K4 không nhắc. Bộ phận DPO chặn trước khi agent đọc ticket Zendesk.
2. **Postgres, bộ dữ liệu chính, không vào được bằng manifest.** Hoặc chờ một provider Go, hoặc chấp nhận `psql` với "chỉ đọc = quyền credential".
3. **Sự kiện từ Slack/Stripe/Zendesk không gắn vào được cửa** đúng cách (không echo `challenge`, không xác minh chữ ký riêng từng bên). Chain không được đánh thức bởi việc thật của công ty.

## 3. Adopter B: ngân hàng khu vực, core banking sau VPN, nhắn tin kiểu Swift, kiểm soát thay đổi kiểu SOX, hội đồng lao động, cơ quan quản lý đòi người duyệt

Assumption: nhắn tin là Swift qua Alliance Access hoặc Lite2, vì đề bài nói "SWIFT-like" và đó là hai giao diện phổ biến. Assumption: hội đồng lao động theo luật Đức (BetrVG), vì đó là chế độ đồng quyết định thường được dẫn; nơi khác thì thay điều luật.

| Bước | Trạng thái | Bằng chứng / mảnh thiếu |
|---|---|---|
| Phê duyệt đưa vào dùng | Không (trước ngày 0) | BetrVG §87(1) Nr. 6: hội đồng có quyền đồng quyết định với "technischen Einrichtungen, die dazu bestimmt sind, das Verhalten oder die Leistung der Arbeitnehmer zu überwachen" ([gesetze-im-internet](https://www.gesetze-im-internet.de/betrvg/__87.html)). Remit ghi "ai đã làm" mọi hành động của thành viên (K1, K2 `gate_calls`): cần thoả thuận trước khi bật. Blueprint 6.6 có "gói hội đồng lao động"; `remit-design.md` không có đơn vị nào sinh gói đó. |
| Cấp cell | Không (trên nền tảng của ngân hàng) | Ngân hàng chạy K8s doanh nghiệp: jail cần `CLONE_NEWUSER` (`internal/jail/plan_linux.go:37`), và trong container chỉ chạy với `seccomp=unconfined` và `apparmor=unconfined` (ghi chú chạy test Linux của dự án). Pod Security Standard Baseline cấm cả hai ([kubernetes.io](https://kubernetes.io/docs/concepts/security/pod-security-standards/)). Không có image (`deploy.md:153`). OS user mỗi cell (K3) không có nghĩa trong pod. |
| Định danh, vai trò | Không | Không SSO SAML/OIDC; Firebase là dịch vụ Google ngoài ngân hàng (`deploy.md` mục 1). Vai trò K1 (`owner/officer/operator/viewer`) không có **maker-checker**: một người duy nhất merge (`AGENTS.md`, rail merge). Kiểm soát thay đổi kiểu SOX cần tách người duyệt khỏi người đưa lên production và bằng chứng phê duyệt; remit không deploy (`design.md:38`) nên phần deploy nằm ngoài, tốt, nhưng duyệt hai người thì thiếu. |
| Forge | Không (thường gặp) | Chỉ GitLab/GitHub (`main.go:99`). Ngân hàng hay dùng Bitbucket Data Center hoặc Azure DevOps (LOW, phổ biến nhưng tôi chưa có số). Forge không phải điểm mở rộng trong thiết kế. |
| Core banking sau VPN | Không | Executor của cổng nằm trong tiến trình cell (`source/doc.go`, The gate). Hệ thống chỉ với tới từ mạng nội bộ đòi cell nằm trong mạng đó, mà cell cần ra Internet tới Anthropic (`deploy.md:40`). Cần **executor từ xa**: phần thực thi cổng chạy trong mạng ngân hàng, nối ra ngoài một chiều, credential chỉ ở đó (đúng luật ADR-0018 "credential exists only inside the executor"). Không K nào phủ. Giao thức core (MQ, ISO 8583, file batch, mainframe) không biết được khi chưa biết nhà cung cấp; tôi không đoán. |
| Swift | Không (và không nên) | Alliance Access cần SAG/SNL, HSM tại chỗ ([Microsoft](https://learn.microsoft.com/en-us/azure/architecture/example-scenario/finance/swift-on-azure-vsrx)); khung CSCF đòi thành phần nằm trong "secure zone" và giới hạn Internet cho chúng ([AWS whitepaper](https://docs.aws.amazon.com/whitepapers/latest/swift-customer-security-controls-framework-2021/swift-environment-protection.html)). Một agent có đường tới model ngoài không thể vào secure zone. Đây là R10 thật: ngoài ranh giới. Thiết kế đúng khi để ngoài, nhưng nên nói rõ cho Swift. |
| Người duyệt quyết định ảnh hưởng khách | Thiếu (K2, K10) | `grant.NeedsOwner` chỉ xét `Effect > Read` hoặc kết nối được bảo vệ (`grant/grant.go:95`). Ở chế độ `-autonomous` (mặc định `true`, `main.go:96`) Chair trả lời yêu cầu quyền thay owner (`design.md:281-285`). Cần một lớp dữ liệu/chủ thể "ảnh hưởng khách" trong `Action` mà pack đặt thành `Ask` cho vai trò người, không cho Chair, và chữ ký K9 ghi ai duyệt. |
| Agent làm được ngày 0 | Rất ít | Nếu repo nội bộ trên GitLab: sửa code và mở draft MR. Không đọc core, không đọc Swift, không SSO. |
| Luật, nhà cung cấp ICT | Thiếu (K10) | Nếu ở EU: DORA đòi sổ đăng ký hợp đồng ICT và chiến lược rời bỏ được thử cho chức năng quan trọng (Art. 28(3), 28(8), [springlex](https://www.springlex.eu/packages/dora/dora-regulation/article-28/)). Một nhà cung cấp model duy nhất cho worker (K4) là rủi ro tập trung; blueprint RR7 đã ghi. |
| Dừng | Chạy (trên VM) | K3 đủ trên VM Linux; không chạy trên K8s như trên. |
| Sao lưu | Thiếu (K11) | Cần đích trong ngân hàng, mã hoá bằng KMS/HSM của họ; K11 nói "ngoài người dùng và ngoài đĩa", chưa nói khoá của ai. |
| Nâng cấp | Thiếu | SOX-like: mỗi bản remit mới cũng là một thay đổi phải duyệt; K8 có `Release` cho thay đổi của agent, không cho chính engine. |
| Rời đi | Thiếu | DORA exit strategy cần xuất được và chạy song song: xem mục 8. |

**Ba điểm gãy đầu tiên khi dùng thật (B):**
1. **Không triển khai được trên nền tảng của ngân hàng** (PSS Baseline cấm seccomp/AppArmor unconfined mà jail cần), và không SSO: dự án dừng ở phòng an ninh trước ngày 0.
2. **Không với tới hệ thống lõi** sau VPN mà không đặt cả cell (có Internet tới model) vào mạng nội bộ; Swift không bao giờ vào được, và không nên.
3. **Không có duyệt hai người và lớp "ảnh hưởng khách"**: yêu cầu của cơ quan quản lý không biểu diễn được; `-autonomous` mặc định bật cho Chair quyền thay người.

## 4. Adopter C: thương hiệu thương mại điện tử 10 người ở Việt Nam, Shopify, cổng thanh toán VN, Zalo, quảng cáo Facebook, hộp thư hỗ trợ tiếng Việt

| Bước | Trạng thái | Bằng chứng / mảnh thiếu |
|---|---|---|
| Cấp cell | Chạy (laptop/VPS) | Một VPS Linux hoặc Mac, `-sign-in=local` (`main.go:119`; cần loopback, đi qua Tailscale như host phát triển). Cần người biết Go/Node để build (`deploy.md`, What the machine needs): một shop 10 người thường không có. |
| Owner, thành viên | Chạy (một owner) | Đúng mô hình một owner. Nhân viên CSKH không có vai trò dùng được trừ khi K1 xong. |
| Loại việc | **Không** | Mọi playbook dựng sẵn là việc phần mềm (`internal/orch/playbooks.go:17-113`: discovery-fix, feature, bug-sweep, wrong-number, backfill, release...); mục tiêu là "every result is a reviewable draft MR" (`design.md:35`). Shop không có repo (có thể có theme Shopify trên GitHub, LOW). Trả lời khách, chỉnh giá, hoàn tiền, đổi ngân sách quảng cáo không có loại kết quả nào. Cần một **kết quả "đề xuất hành động"**: một gói lời gọi ghi đã qua cổng, owner duyệt một lần, executor chạy. K5 chỉ có Ask theo từng lời gọi. |
| Shopify | Thiếu (K5, phải mở rộng) | REST Admin là legacy từ 2024-10-01, app public mới chỉ dùng GraphQL từ 2025-04-01 ([shopify.dev](https://shopify.dev/docs/api/admin-rest)). GraphQL là **một** endpoint POST cho cả đọc và ghi: "mỗi endpoint một `effect`" của manifest (`remit-design.md:108`) không phân lớp được; cần bộ phân loại theo operation (query/mutation, tên mutation), như guard SQL. Đây đúng là lỗi `ReadPosts` của Grafana (`source/doc.go`, Scripts). |
| Cổng thanh toán (VNPay làm ví dụ) | Thiếu (K5) | `querydr`/`refund` là POST JSON với `vnp_SecureHash`: HMAC trên chuỗi tham số nối bằng `|` với khoá bí mật của merchant ([VNPay](https://sandbox.vnpayment.vn/apis/docs/truy-van-hoan-tien/querydr&refund.html)). Mẫu "tên biến credential vào header" không biểu diễn được ký theo từng request; cần lược đồ xác thực "hmac-fields" chạy trong executor. Hoàn tiền là H2. |
| Zalo OA, ZNS | Thiếu (K5) | OAuth v4 với refresh token (`oauth.zaloapp.com/v4/oa/access_token`, theo SDK cộng đồng, [unpkg](https://unpkg.com/@warriorteam/redai-zalo-sdk@1.11.1/docs/AUTHENTICATION.md); tôi chưa đọc được trang chính thức, MEDIUM). Manifest không có luồng làm mới token; credential "gõ vào trường chỉ ghi" một lần (ADR-0017) không hợp với token hết hạn. Tin ZNS chỉ gửi qua mẫu đã được Zalo duyệt ([Infobip](https://www.infobip.com/docs/zalo/compliance-guidelines.md)): gửi tin là H2 và có phí. |
| Facebook Ads | Thiếu (K5) | Marketing API cần system user token, `ads_management` cần App Review, giới hạn tốc độ theo Business Use Case ([Meta](https://developers.facebook.com/docs/marketing-api/get-started/authorization)). Manifest REST biểu diễn được phần đọc; thiếu giới hạn tốc độ theo điểm. Webhook Meta xác minh bằng **GET** `hub.challenge` ([Meta](https://developers.facebook.com/docs/graph-api/webhooks/getting-started)); cửa chỉ `POST` (`door.go:26`). Chữ ký `X-Hub-Signature-256` thì cửa đã đọc (`door.go:120`). |
| Hộp thư hỗ trợ tiếng Việt | Thiếu | Nếu là email: IMAP/SMTP, không phải REST/CLI/MCP chuẩn (MEDIUM). Nếu là Zalo/Messenger: như trên. Agent đọc tiếng Việt được; viết trả lời không có đường ghi. Thông báo "đây là AI" của K10 phải bằng tiếng Việt và có ở mọi công cụ gửi ra. |
| Agent làm được ngày 0 | Gần như không | Không nguồn nào trong `kinds` (`source.go:33-46`) là hệ thống của shop. Agent chỉ có lịch sử chain, outcomes, playbook. |
| Luật (Việt Nam) | Thiếu (K10, U7) | Luật Bảo vệ dữ liệu cá nhân 91/2025/QH15 hiệu lực 2026-01-01; chuyển dữ liệu ra nước ngoài cần hồ sơ đánh giá tác động nộp Bộ Công an trong 60 ngày ([Conventus](https://conventuslaw.com/report/vietnam-special-alert-new-personal-data-protection-law-demands-business-actions/)). Doanh nghiệp nhỏ được miễn DPIA và bộ phận bảo vệ dữ liệu 5 năm, **trừ** khi xử lý dữ liệu nhạy cảm hoặc số lượng lớn chủ thể ([Tilleke](https://tilleke.com/insights/vietnams-new-personal-data-protection-law-a-closer-look)). Mọi lời gọi model mang tin nhắn khách là chuyển ra nước ngoài (không có vùng model ở Việt Nam, MEDIUM): pack VN phải sinh hồ sơ đó. Nghị định 53/2022 về lưu trữ trong nước áp dụng cho doanh nghiệp cung cấp dịch vụ trên mạng ([Tilleke](https://www.tilleke.com/insights/decree-53-provides-long-awaited-guidance-on-implementation-of-vietnams-cybersecurity-law/)); có phủ một shop hay không là câu hỏi luật sư (LOW). |
| Dừng, sao lưu, nâng cấp | Như A | K3, K11 phủ; một shop không có người vận hành để chạy diễn tập. |
| Rời đi | Thiếu (U7) | Xem mục 8. |

**Ba điểm gãy đầu tiên khi dùng thật (C):**
1. **Không có loại kết quả cho việc của shop.** Chain kết thúc bằng báo cáo hoặc draft MR; việc thật (trả lời khách, hoàn tiền, đổi ngân sách) vẫn do người làm tay.
2. **Không hệ thống nào của shop vào được bằng manifest như thiết kế:** Shopify GraphQL, ký HMAC của cổng VN, refresh OAuth của Zalo đều cần lược đồ mà K5 không có. Lối thoát
   của ADR-0017 ("the owner asks the Chair or the Assistant to write the source") nghĩa là shop phải giữ một fork Go của engine, build lại, và vá xung đột mỗi lần nâng cấp.
3. **Không thấy chi phí.** Hệ không ghi chi phí (blueprint RR4, ADR-0010); `-max-concurrent 2` (`main.go:101`) là giới hạn duy nhất. Với 10 người, hoá đơn model một tháng là quyết định
   sống còn mà owner chỉ biết khi đọc hoá đơn.

## 5. (1) Số connector manifest và hệ thống không biểu diễn được bằng REST, CLI hay stdio MCP

Đếm = hệ thống cần thêm ngoài các provider đã có (`source.go:33-46`). Ước lượng, MEDIUM.

| Adopter | Manifest cần | Đã có | Không phải REST/CLI/MCP, hoặc REST mà manifest K5 không biểu diễn được |
|---|---|---|---|
| A | 5–6: Stripe, HubSpot, Zendesk, Slack, AWS (CLI), Postgres (CLI hoặc Go) | GitHub, CloudWatch; Bytebase nếu họ dùng | Postgres (giao thức dây); Slack Socket Mode (WebSocket); AWS ngoài CloudWatch (SigV4, chỉ qua CLI); webhook Stripe/Slack/Zendesk (lược đồ ký riêng) |
| B | 6–10 (chưa biết nhà cung cấp core) | Có thể Datadog/Grafana nếu họ dùng | Swift (SAG/SNL, HSM, secure zone; ngoài ranh giới); core banking nếu là MQ/ISO 8583/file batch/mainframe; LDAP/AD (giao thức LDAP); mọi thứ sau VPN (cần executor từ xa) |
| C | 5–6: Shopify, cổng thanh toán (1–2), Zalo OA/ZNS, Meta Ads, hộp thư | Không | Email IMAP/SMTP; Shopify GraphQL (REST về hình thức, nhưng một endpoint đọc lẫn ghi); ký HMAC tham số (VNPay); OAuth refresh xoay vòng (Zalo, LOW về chi tiết); xác minh webhook GET (Meta) |

Kết luận: **"REST, CLI, MCP stdio" là đúng về giao vận, sai về xác thực và phân lớp.** Phần lớn hệ thống thật là HTTP, nhưng mỗi bên có lược đồ ký, làm mới token, phân trang,
giới hạn tốc độ và xác minh webhook riêng; manifest dạng dữ liệu cần một **tập đóng các lược đồ xác thực và phân loại viết bằng Go** để dữ liệu chọn, nếu không mỗi hệ
thống lại là một file Go như hôm nay. Ngoài ra K5 nói "stdio MCP qua `jailRunner`" nhưng ADR-0017 đã xoá thư viện MCP server và "chưa có MCP client" (`remit-design.md:109`): đưa MCP lạ
trở lại là đảo ADR-0017, phải được nói và lập ADR mới.

## 6. (2) "Một cell mỗi công ty" trên laptop, một VPS, và K8s doanh nghiệp

| | Laptop | Một VPS | K8s doanh nghiệp |
|---|---|---|---|
| A | Thử nghiệm được; máy ngủ thì routine dừng; dữ liệu khách EU trên laptop khó qua GDPR Art. 32 (MEDIUM) | **Hợp nhất**: VPS vùng EU, Caddy (`deploy.md`), systemd (chưa từng chạy, `deploy.md:87`), backup ra S3 EU (K11) | Không chạy dưới PSS Baseline (jail cần unconfined); SQLite một người ghi nên một replica, PVC RWO, không HA; chưa có image (`deploy.md:153`) |
| B | Không (chính sách thiết bị, secure zone) | Chỉ khi VM nằm trong mạng ngân hàng và được mở ra tới model; phần lớn ngân hàng không cho (LOW) | Nền tảng họ chọn, và chính chỗ gãy: PSS, không image, OS user mỗi cell vô nghĩa, `KillMode=process` (`deploy.md`, Keep it running) phải thành hành vi PID 1 trong container |
| C | Hợp với một chủ shop dùng Mac; máy tắt đêm thì chain đứng | **Hợp nhất**, nếu có người dựng và cập nhật | Quá sức với 10 người |

Cell đúng cho A và C trên VPS. Với K8s, cần một **adapter jail "pod"** (jail dựa vào sandbox của runtime như gVisor/Kata thay vì user namespace tự tạo, LOW: chưa kiểm khả thi) và image chính thức;
đó đúng là điểm mở rộng thứ năm ("một adapter jail") nhưng thiết kế chỉ nghĩ theo OS, không theo môi trường container.

## 7. (3) Kế hoạch model theo ràng buộc vùng dữ liệu và nhà cung cấp

K4: judge đa nhà cung cấp qua API trước; worker chỉ Claude Code. Claude Code tự hỗ trợ Bedrock, Vertex (Agent Platform), Foundry và LLM gateway
([Claude Code](https://code.claude.com/docs/en/third-party-integrations)) nhưng Remit không dùng được vì lọc môi trường (`childenv.go:33-61`) và quy ước "no API key" (`deploy.md:37`).

| Adopter | Ràng buộc | Kế hoạch K4 có đáp được? | Mảnh thiếu |
|---|---|---|---|
| A | Inference trong EU; DPA với nhà cung cấp | Judge: được nếu chọn host EU. Worker: **không**, API trực tiếp chỉ `global`/`us` ([data residency](https://platform.claude.com/docs/en/manage-claude/data-residency)) | Cấu hình backend worker theo cell (Bedrock `eu.*` hoặc Vertex EU), credential cloud là "credential duy nhất phiên giữ" thay cho login (cùng hạng như `childenv.go:31`), ghim model (`ANTHROPIC_DEFAULT_*_MODEL`, cùng trang) |
| B | Có thể đòi on-prem hoặc cloud đã phê duyệt; DORA tập trung nhà cung cấp; lưu giữ | Judge open-weight on-prem: được. Worker: chỉ Anthropic qua một trong bốn đường; không đường on-prem. Anthropic giữ input/output API tới 30 ngày trừ khi có ZDR ([privacy](https://privacy.claude.com/en/articles/7996866-how-long-do-you-store-my-organizations-data)) | Như A, cộng PrivateLink/gateway trong tenancy của ngân hàng; một worker thứ hai thật sự (K4 "sau") trở thành điều kiện, không phải tuỳ chọn |
| C | Mọi lời gọi là chuyển ra nước ngoài theo Luật 91/2025; ngân sách | Được về kỹ thuật; về luật cần hồ sơ đánh giá chuyển dữ liệu | Pack VN sinh hồ sơ; hiển thị chi tiêu |

Điều khoản: chạy Claude Code trong sản phẩm cho người khác thì mỗi người dùng cuối phải tự xác thực bằng khoá API, gói Claude hoặc credential 3P của chính họ; bên cung cấp
không được trả tiền hay làm trung gian cho lượng dùng đó ([legal](https://code.claude.com/docs/en/legal-and-compliance)). Cell mỗi công ty với login của chính công ty khớp điều này
(HIGH về văn bản, MEDIUM về áp dụng). Cùng trang: giới hạn của gói Pro/Max "assume ordinary, individual usage": agent chạy 24/7 cho cả công ty nên dùng API hoặc 3P, nghĩa là
đường Bedrock/Vertex không phải tiện ích mà là đường mặc định cho doanh nghiệp.

## 8. (4) Rời đi: công ty có mang dữ liệu đi và xoá sạch được không

**Hôm nay: không, và thiết kế mới hứa một nửa.** K1 có `tenant export|delete`, K10 có "xuất và xoá theo cell". Thiếu:

- **Kho dữ liệu nằm rải rác.** Ngoài `remit.db`: `work/` (worktree, 9.7 GB trên host phát triển, `deploy.md:114`), transcript Claude Code ở `<data>/claude/projects/`
  (gói state không mang theo, `deploy.md:113-114`), `remit.log`, `agents.log`, `backups/`, khoá sao lưu ở `-key-backup-dir`, bản sao ngoài đĩa (K11), neo băm gửi ra ngoài (K2),
  nhánh `feature/mc-*` đã đẩy lên forge, và dữ liệu ở nhà cung cấp model (tới 30 ngày, trên). `tenant delete` phải liệt kê từng kho và in phủ: "đã xoá 7/9, còn forge và nhà cung cấp, vì...".
- **Chuỗi băm đụng quyền xoá.** `gate_calls` băm nối (K2) và `events` "never deleted" (`store/pack.go:17`). Xoá một hàng chứa dữ liệu cá nhân làm gãy chuỗi. Cách nhỏ nhất: chuỗi băm
  trên **digest** của hàng, nội dung cá nhân để ngoài chuỗi và xoá được; xác minh vẫn chạy trên digest. Phải quyết trước U2, không sau (migration một lần).
- **Định dạng xuất.** Chỉ có file SQLite và lược đồ nội bộ; không có định dạng trung tính có tài liệu (JSONL theo thực thể cộng manifest). Với B (DORA exit strategy phải thử được) đây là điều kiện.
- **Mã hoá theo cell.** Một khoá `secret.key` mỗi cell đã cho phép "xoá bằng huỷ khoá" cho credential; mở rộng cho nội dung sự kiện thì xoá cả backup cũ bằng huỷ khoá (crypto-shredding) mà không cần với tới từng bản sao.

## 9. (5) Mười tính năng thiếu quan trọng nhất cho tính phổ quát, xếp hạng

| # | Thiếu | Ai bị chặn | Thay đổi thiết kế nhỏ nhất |
|---|---|---|---|
| 1 | **Loại kết quả không phải code**: gói hành động ghi đã qua cổng, owner duyệt một lần | C hoàn toàn, A một nửa | Thực thể `Proposal` theo mẫu Decision/Change: danh sách `Action` với `params_digest`, executor chạy sau khi owner Apply; playbook vận hành (support, refund, ads) viết chung, không theo công ty. K5/K9 |
| 2 | **Lược đồ xác thực và phân loại cho manifest** | A, C | Tập đóng trong Go: `bearer`, `basic`, `hmac-fields`, `sigv4`, `oauth2-refresh` (token làm mới trong executor, không trong trường chỉ ghi); bộ phân loại operation cho GraphQL. Manifest chỉ chọn tên. K5 |
| 3 | **Backend worker theo cell** (Bedrock/Vertex/Foundry/gateway, vùng, ghim model) | A, B | Bảng `settings` của cell giữ backend; `childenv` thêm đúng các biến backend đó; catalog `provider` thêm trường vùng. K4 |
| 4 | **Định danh và phê duyệt**: OIDC/SAML, vai trò từ IdP, maker-checker | B, A | `-sign-in=oidc`; luật "n người, vai trò khác người yêu cầu" trong bảng hành động→vai trò của K1; chữ ký K9 ghi cả hai. K1, K9 |
| 5 | **Forge là điểm mở rộng** | B, nhiều công ty khác | Giao diện `forge` đã có hai hiện thực (`internal/forge`): thêm adapter, catalog repo vào `settings`. U1 |
| 6 | **Executor từ xa cho mạng riêng** | B, mọi hệ thống sau VPN | Một binary executor chạy cổng phía thực thi trong mạng khách, kết nối ra ngoài, credential chỉ ở đó; cổng vẫn quyết ở cell, sổ ghi ở cell. Cần ADR (rail). K2 |
| 7 | **Chạy trong container/K8s** | B, A trên EKS | Image chính thức; adapter jail dựa vào sandbox runtime; StatefulSet một replica mỗi cell. K3, K11 |
| 8 | **Rời đi và xoá đủ** | A, B, C | Kiểm kê kho, chuỗi băm trên digest, xuất JSONL, crypto-shredding (mục 8). K10, U7 |
| 9 | **Nội dung pack luật**: lớp "quyết định ảnh hưởng đáng kể tới một người" (GDPR 22, cơ quan quản lý), dữ liệu về nhân viên (hội đồng lao động), hồ sơ chuyển dữ liệu VN | A, B, C | `Action.Data` thêm chủ thể (`customer`, `employee`) và lớp `significant-decision`; pack sinh hồ sơ (gói hội đồng lao động, hồ sơ chuyển dữ liệu) từ `purposes`. K10 |
| 10 | **Nhận sự kiện từ mọi nơi**: xác minh webhook theo bên gửi, challenge, WebSocket, polling cho connector khai báo | A, C | Cửa nhận lược đồ xác minh theo tên (cùng tập với #2) và trả challenge; `Pollable` cho manifest. Không thêm biểu mẫu |

Ngoài mười mục: hiển thị chi tiêu cho công ty nhỏ (mâu thuẫn trực tiếp với `AGENTS.md` "records none and caps none" và ADR-0010; tôi nêu, không đề xuất bỏ nguyên tắc: tối thiểu là một
nguồn đọc hoá đơn của nhà cung cấp, chỉ đọc, ở Outcomes), và K6 được gọi tên (`remit-design.md:113`) nhưng không có mục riêng.

## 10. (6) "Mọi công ty, mọi hệ thống" đứng được tới đâu

- **Đứng được:** công ty phần mềm nhỏ và vừa, dữ liệu và hệ thống với tới được từ một host có Internet, một owner chịu trách nhiệm merge, luật cho phép gửi dữ liệu tới Anthropic
  ở vùng họ chấp nhận. Đó là A sau U1–U4 cộng mục #2, #3, #10. **MEDIUM.**
- **Đứng một phần:** công ty mà việc chính không phải phần mềm (C). Engine đủ an toàn, nhưng sản phẩm không làm được việc của họ cho tới khi có #1 và #2. Thiết kế hiện viết như
  thể chỉ thiếu connector; thực tế thiếu cả loại kết quả. **HIGH** cho chẩn đoán (playbook và mục tiêu MR trong code), **MEDIUM** cho cách sửa.
- **Không đứng:** tổ chức được quản lý chặt với mạng riêng (B). Chặn không ở model hay luật mà ở nền tảng chạy, định danh, phê duyệt hai người và vị trí executor; Swift thì ngoài ranh
  giới vĩnh viễn, đúng như R10. **HIGH** cho PSS và đăng nhập; **MEDIUM** cho phần còn lại.
- Câu trung thực cho `remit-design.md:11`: *"Remit chạy cho mọi công ty có hệ thống với tới được qua HTTP hoặc CLI từ một host có Internet, chấp nhận Anthropic làm worker ở một vùng có
  sẵn, và có một owner duyệt kết quả; mỗi trục khác (forge, định danh, mạng riêng, loại kết quả, nền tảng container) là một điểm mở rộng chưa có."*

## 11. Assumption, độ chắc, việc tiếp

- Assumption: B dùng Swift qua Alliance Access hoặc Lite2, vì đề bài nói "SWIFT-like" và đó là hai giao diện phổ biến.
- Assumption: hội đồng lao động của B theo BetrVG, vì đó là chế độ đồng quyết định thường được dẫn; nơi khác thì thay điều luật.
- Assumption: cổng thanh toán của C là VNPay hoặc tương tự có ký HMAC tham số, vì VNPay là ví dụ tôi kiểm được tài liệu; MoMo, ZaloPay, PayOS chưa kiểm.
- Assumption: ngân hàng chạy K8s với PSS Baseline trở lên, vì đó là mức tối thiểu Kubernetes khuyến nghị cho workload không đặc quyền.
- Chưa kiểm: thời hạn token Zalo trên trang chính thức (trang không đọc được bằng công cụ); Nghị định 53 có phủ một shop; adapter jail dựa vào gVisor/Kata có thay được user namespace;
  Bitbucket/Azure DevOps phổ biến ở ngân hàng tới mức nào.
- Không chạy code nào; mọi kết luận về hành vi dựa trên đọc code và tài liệu. Việc tiếp: chủ tọa quyết có mở rộng bảng điểm mở rộng ở `remit-design.md:15-21` thành tám trục, và có đặt #1
  (loại kết quả) và chuỗi băm trên digest (mục 8) trước U2 hay không.

## Nguồn

Code và tài liệu trong repo: `docs/remit-design.md`, `docs/design.md`, `docs/deploy.md`, `docs/ai-run-company-blueprint.md` (3.8, 6.6), ADR-0015..0018, `cmd/remit/main.go:89-124`,
`cmd/remit/signin.go:19-22`, `internal/source/{source.go,doc.go,starrocks.go,bytebase.go,form.go}`, `internal/orch/{capabilities.go,playbooks.go,jira.go}`, `internal/library/access.go`,
`internal/childenv/childenv.go`, `internal/door/door.go`, `internal/jail/plan_linux.go`, `internal/forge/forge.go`, `internal/grant/grant.go`, `internal/store/pack.go`.
Ngoài: các URL dẫn tại chỗ (Anthropic/Claude Code, AWS, Kubernetes, Shopify, VNPay, Meta, Slack, Stripe, Swift qua Microsoft và AWS, gesetze-im-internet, gdpr-info, springlex, Conventus, Tilleke, Infobip).
