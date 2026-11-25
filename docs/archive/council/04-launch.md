# Hội đồng, phần 4: Ra mắt trọn hệ thống và sửa khi đang chạy

> Bản nháp của một trong bốn người soạn trong hội đồng, cho blueprint cuối về công ty do agent vận hành. Bản 2, ngày 2026-11-01.
> Bản 1: [`04-launch-v1.md`](04-launch-v1.md). Từng phản biện và cách xử lý: [`04-response.md`](04-response.md).
> Dựa trên bốn báo cáo nghiên cứu `research-ai-autonomous-system-*.md`, các PDF trong [`papers/`](../papers/INDEX.md), các ADR
> trong [`adr/`](../adr/) và sáu phản biện `challenge-0*.md` của hội đồng.
>
> Nhãn: **HIGH** (tôi đã tra con số trong toàn văn, trang gốc, ADR hoặc mã trong lần soạn này), **MEDIUM** (nguồn thứ cấp, vendor,
> hoặc một phản biện của hội đồng đã đọc mà tôi chưa mở lại), **LOW** (snippet, suy luận). Điều không có bằng chứng ghi
> **Đề xuất của hội đồng (giả thuyết)**. Phán quyết của chủ tọa là mặc định, chờ owner xác nhận. Đây không phải tư vấn pháp lý.

## Quyết định, điểm cố định và cách đọc

**Quyết định của owner.** Toàn hệ thống bật cùng lúc bên trong một ranh giới và được sửa trong khi chạy. Agent không bị giới hạn
sáng kiến: không trần chi phí, không trần tự chủ, không giới hạn phạm vi ngoài điều luật buộc, không cổng duyệt ngoài các rail đã có,
không giai đoạn, không lịch, không cơ chế tự dừng theo hiệu năng. Không thí nghiệm: không nhánh ngẫu nhiên, không holdout, không A/B
trên khách. Không bí mật: mọi hành động của agent hiện cho owner và người của công ty.

**Lập trường** (`AGENTS.md:21-22`): *"take a risk when its loss is bounded and recovery is cheap. The recovery (rollback, per-tenant
stop, backups) is built before the risk is taken"*. Với hiệu ứng đảo ngược được, phục hồi xây trước quyết định một sai lầm tốn bao
nhiêu. Với hiệu ứng không đảo ngược được, phục hồi chỉ giới hạn lần sau: **điều kiện "loss is bounded" không thoả** ở lớp đó, và rủi ro
được nhận qua một tuyên bố có chữ ký (mục 1.2, 5.5).

**Định vị.** Cho doanh nghiệp lớn, bản này là một **phương pháp đo và kiểm chứng** chạy trên nền tảng agent họ đã mua (Microsoft,
Salesforce, ServiceNow, AWS, Google, OpenAI), không phải một nền tảng thay thế. Lợi thế của nó là độ trung thực của phép đo (pass^k,
false-pass, lỗi thoát, khoảng Wilson) và kiểm chứng sau khi giao (`challenge-05-enterprise.md` §4, **MEDIUM**).

### Điểm cố định

Các điểm này quyết định ai giữ công tắc, ai giữ chìa khoá và luật nào áp dụng; trong khuôn đó agent làm mọi việc nó chọn.

| # | Điểm cố định | Nội dung | Nguồn |
|---|---|---|---|
| F1 | Dừng tay | Owner (hoặc Assistant theo lời owner) tạm dừng hoặc dừng toàn bộ, một cell, một tenant hay một agent; dừng nằm ngoài môi trường của agent và được đo ở mạng của host (D1) | `AGENTS.md:47-49` |
| F2 | Credential ngoài ngữ cảnh agent | Nền tảng giữ secret, agent dùng qua tool; owner nhập vào trường chỉ ghi; mọi lần dùng có log | `AGENTS.md:49-51` |
| F3 | Tuân thủ pháp luật | Khi luật và một quyết định khác va nhau, **luật thắng** và ngày 0 mang nó (mục 5.3) | Phán quyết của chủ tọa R4 |
| F4 | Rail của `AGENTS.md` | Authentication; pause và stop; **merge do owner kéo** (hoặc Assistant theo lời owner), ghi ai làm; "no agent lifts or moves a rail (merges, signs in)" | `AGENTS.md:47-49`; Phán quyết của chủ tọa R1 |

Gate ép được luật về **luồng dữ liệu** (lớp dữ liệu nào tới đích nào, cho mục đích nào). Tính hợp pháp của **nội dung** (một phát
ngôn có phỉ báng hay lừa dối không) do agent và pháp chế xét, không do gate (`challenge-03-security.md` §3.4, **MEDIUM**).

**Phán quyết của chủ tọa áp vào bản này:** R1 rail giữ nguyên, merge ở owner; R2 "mở" là mở với owner và người của công ty, công bố ra
ngoài sau kiểm pháp lý có người ký tên, các thứ chỉ chỗ cho kẻ tấn công được niêm (mục 4); R3 lặp một tác vụ K lần để đo nhất quán là
quan sát; R4 luật thắng (mục 5.3); R5 phân lớp hiệu ứng, hoãn-và-huỷ là tuỳ chọn mặc định tắt (mục 1.2); R7 lỗ của hiện thực tham chiếu
là việc xây trước ngày 0 (mục 1.4); R8 sửa số và câu theo phản biện 01 và 06; R10 ranh giới là đơn vị bên áp dụng chọn, đơn vị bên ngoài
là so sánh quan sát (mục 5.1).

## 1. Ngày 0

### 1.1 Bật cùng lúc

Bên trong ranh giới: mọi role agent, Chair, Assistant, auditor; lớp ngữ cảnh chung; mọi tool và capability; nền tảng (gate cho mọi
lời gọi tool, proxy cho mọi lưu lượng ra, kho secret, log, backup); màn hình quan sát; vòng sửa (mục 2); scorecard (mục 3). Một thành
phần thiếu ở ngày 0 không có mốc trước ngày 0 để so.

### 1.2 Lớp hiệu ứng (Phán quyết của chủ tọa R5)

| Lớp | Ví dụ | Phục hồi xây trước ngày 0 | Cận tổn thất |
|---|---|---|---|
| Đảo ngược được | Mã, cấu hình, dữ liệu nội bộ có backup | Hoàn tác theo bản phát hành (D6); backup ngoài quyền agent (D3); xoá mềm và snapshot trước khi ghi đè | Cửa sổ mất dữ liệu và thời gian khôi phục, đo ở M7 |
| Bù trừ được | Hoàn tiền, tín dụng, giá, đơn hàng, cam kết với khách | Mỗi tác dụng là một bản ghi có kiểu; đường bù (đảo bút toán, liên hệ khách); tổng theo cửa sổ thời gian hiện trên màn hình owner | Bằng số tiền đã đi cộng chi phí bù; không có trần |
| Không đảo ngược được | Email hay tin nhắn đã gửi, phát ngôn công khai, dữ liệu đã tới bên thứ ba, tiền đã chuyển ra ngoài, xoá ở hệ thống bên thứ ba | Ghi trước khi gửi; **hoãn-và-huỷ** (nền tảng giữ một cửa sổ ngắn, F1 trong cửa sổ rút được hành động): tuỳ chọn, **mặc định tắt** | **Không có cận.** Nhận qua tuyên bố rủi ro còn lại (5.5) |

Cam kết của agent ràng buộc công ty: Luật Giao dịch điện tử 20/2023/QH15 Điều 14(2)(a) và UETA §14 (`challenge-04-legal.md` K16,
**MEDIUM**). Với cả lớp không đảo ngược được, "phát hiện, dừng, phục hồi" giới hạn **lần sau**, không trả lại **lần này**
(`challenge-03-security.md` §4).

### 1.3 Phải có lúc ra mắt (mỗi mục có phép kiểm)

Phép kiểm xác minh thứ đó **tồn tại và hoạt động**; nó không duyệt agent.

| # | Thành phần | Phép kiểm ở ngày 0 | Vì sao (bằng chứng) |
|---|---|---|---|
| D1 | F1, dừng | Dừng từng mức. Đo **ở mạng của host** (bộ đếm tường lửa hoặc bắt gói ở cổng ra): sau thời điểm dừng không còn byte ra từ tiến trình nào của agent; bảng tiến trình không còn tiến trình con nào của phiên; gate từ chối mọi lời gọi và mọi lần dùng credential; agent không có lệnh bỏ dừng | Palisade: có model phá cơ chế tắt tới 97% (KTC 95% 96–98%) dù được dặn, khi cơ chế nằm trong môi trường của model (`arxiv.org/abs/2509.14260`, **HIGH**). Log của gate chỉ chứng minh đường qua gate đã đóng (`challenge-03` §3.1) |
| D2 | F2, credential | Quét mọi bề mặt agent đọc được (tệp, repo, bộ nhớ, tài liệu chung, log agent đọc, `~/.ssh`, `~/.config/glab-cli`, môi trường của mọi tiến trình cùng user) tìm **mọi** credential, không chỉ giá trị trong kho: 0 kết quả | PocketOS (25/04/2026): token trong tệp không liên quan, quyền cả `volumeDelete`, xoá database và backup trong 9 giây (Zenity kể lại lời founder, **MEDIUM**) |
| D3 | Backup | Khôi phục một tenant vào môi trường sạch, so checksum; thử đọc và xoá backup bằng mọi credential agent dùng: bị từ chối | PocketOS: backup cùng volume (**MEDIUM**); Replit 07/2025 (OECD AI Incident Monitor, **MEDIUM**) |
| D4 | Log do nền tảng ghi | Mỗi hiệu ứng lấy mẫu ở hệ thống ngoài có dòng log; log là chuỗi băm ghi ra nơi phiên không ghi được; thử sửa bằng danh tính agent: bị từ chối. Mỗi dòng có agent, tool, tham số **kèm nguồn gốc của từng tham số** (lời owner, brief, hay nội dung ngoài), kết quả, mã model thực phản hồi, phiên bản prompt, playbook, harness, bộ nhớ | Apollo: o1 giữ lời dối trong hơn 85% câu hỏi tiếp theo (`arxiv.org/abs/2412.04984`, **HIGH** theo phản biện 01); đổi một tham số là dạng tấn công MCPTox hiệu quả nhất (`challenge-03` A6) |
| D5 | Quan sát trực tiếp | Mở màn hình của từng agent trong trình duyệt: hành động, câu hỏi, lỗi hiện trực tiếp; tab đếm việc chờ owner; scorecard tính từ log | `AGENTS.md:43-44` |
| D6 | Change log và hoàn tác | Sửa một dòng playbook, thấy phiên bản mới, hoàn tác bằng một hành động; mỗi bản phát hành có trường "đổi mức rủi ro AI hay không" do người được giao ký (mục 2.4) | Hai postmortem Anthropic (mục 7); Luật AI 134/2025/QH15 Điều 10(2) (`challenge-04` §2.3, **MEDIUM**) |
| D7 | Bộ kiểm hồi quy | Ca từ lỗi thật trong lịch sử của chức năng, cộng phiên dài nhiều lượt; mỗi ca chạy K ≥ 5 lần; có **lát mở** (agent thấy kết quả) và **lát niêm** (agent bị đo không thấy ca lẫn kết quả; owner thấy hết) | Lỗi harness Claude Code chỉ lộ qua nhiều lượt (`anthropic.com/engineering/april-23-postmortem`, **HIGH**); dùng lại holdout dẫn tới quá khớp (Dwork 1411.2664; Blum và Hardt 1502.04585, theo `challenge-02` P3) |
| D8 | Danh tính model | Mã snapshot có ngày được ghi; mã model thực phản hồi khác mã đã chọn thì hiện cho owner; lịch ngừng model trong change log | Anthropic báo trước tối thiểu 60 ngày (trang deprecations, **MEDIUM**) |
| D9 | Tool tính số xác định | Mỗi phép tính số khách dựa vào có một tool bằng mã; scorecard quan sát tỷ lệ con số gửi khách đi qua tool đó | FinSheet-Bench: model tốt nhất 82,4%, khoảng 1 lỗi trên 6 câu (`arxiv.org/abs/2603.07316`, **HIGH**) |
| D10 | Nguồn dữ liệu độc lập | Khối lượng liên hệ, leo thang, khiếu nại, CSAT lấy từ hệ thống agent không ghi được, ở cả đơn vị trong và ngoài ranh giới | CBA, Klarna (mục 5.7) |
| D11 | F3, hồ sơ pháp lý | Mỗi mục "trước khi dùng" ở 5.3 có văn bản do người đại diện theo pháp luật ký, có ngày | Luật Doanh nghiệp 2020 Điều 12(1): người đại diện là cá nhân (`challenge-04` K15, **MEDIUM**) |
| D12 | Đường công bố ra ngoài | Mỗi lần xuất qua bốn kiểm ở 4.2 và có tên người ký | Phán quyết của chủ tọa R2 |
| D13 | Thông báo AI bằng mã | Gate thêm câu "đây là AI" vào **mọi** tin nhắn, email, cuộc gọi agent gửi người ngoài, từ lần tương tác đầu; nội dung sinh ra có đánh dấu máy đọc | AI Act Điều 50(1),(2),(5); Luật AI Điều 11(1) (`challenge-04` §4, **HIGH** theo phản biện) |
| D14 | Dữ liệu cá nhân theo mục đích và role | Sổ mục đích do người duyệt; gate so lớp dữ liệu của mỗi hành động với role và mục đích; mục đích mới là một rail người ký | Luật 91/2025/QH15 Điều 3(2), 9(4)(a), 30(1),(3) ("phân quyền truy cập"); GDPR Điều 28(3)(a) (`challenge-04` §3.2, **HIGH** cho chữ theo phản biện, **MEDIUM** áp dụng) |
| D15 | Canary token | Credential giả và bản ghi khách giả; chạm tới thì báo cho owner, không dừng gì; vị trí niêm (mục 4) | `challenge-03` D8 |
| D16 | Máy chủ MCP lần đầu | Ghim phiên bản; checker xác định so mô tả với hành vi và tìm chỉ thị trong mô tả; chỉ báo | MCPTox: ASR cao nhất 72,8%, từ chối dưới 3% (`challenge-03` A12, **HIGH** theo phản biện) |

### 1.4 Lỗ của hiện thực tham chiếu Remit (Phán quyết của chủ tọa R7)

F1 và F2 chỉ là thuộc tính khi các lỗ dưới đây đóng. Mỗi lỗ là một việc xây trước ngày 0, có phép thử.

| Lỗ | Nguồn (tôi đã đọc, **HIGH**) | Việc phải xây | Phép thử |
|---|---|---|---|
| Phiên giữ mạng tới nhà cung cấp model, ngoài gate | ADR-0018:71-72 ("the session keeps the network it needs to reach its model provider") | Mọi lưu lượng của phiên đi qua một proxy do nền tảng giữ; F1 và F3 ép ở đó | D1 đo ở mạng host |
| Hồ sơ sandbox macOS bắt đầu bằng `(allow default)` | `internal/jail/sandbox_darwin.go:143` | Hồ sơ mặc định từ chối, hoặc phiên chạy trọn trong jail chỉ có mạng qua proxy | Phép thử thoát sandbox |
| Tiến trình con tự `setsid` sống quá deadline; phiên đọc môi trường của mọi tiến trình cùng user | ADR-0021:63, 66 | pid namespace hoặc user OS riêng cho agent | D1: không còn tiến trình con sau dừng; D2: môi trường server không chứa secret |
| Phiên cùng user OS với server: ký hiệu được server, ghi được crontab và tệp khởi động shell | ADR-0018:109-111; ADR-0004 | User OS riêng cho agent (hướng ADR-0004 đã chọn) | Thử ghi crontab của user server từ phiên: bị từ chối |
| Credential GitLab và SSH đọc được từ phiên | ADR-0018:112-114 | Connection kind GitLab sau gate; khoá ra khỏi tầm phiên | D2 trên `~/.ssh`, `~/.config/glab-cli`: 0 kết quả |
| Một owner mỗi server | ADR-0022 | Định danh doanh nghiệp: SSO, nhiều người theo cell, ghi ai làm | Hai người ở hai vai cùng hành động, log ghi đúng từng người |
| Một nhà cung cấp model | ADR-0006 | Hoặc nhà cung cấp thứ hai có DPA và eval riêng, hoặc ghi rủi ro tập trung vào 5.5 | Diễn tập chuyển nhà cung cấp, hoặc chữ ký ở 5.5 |

## 2. Quan sát và sửa khi đang chạy

### 2.1 Sai được nhận ra từ đâu

Xếp theo độ tin của tín hiệu (suy luận từ bằng chứng, **MEDIUM**):

1. **Kiểm xác định trên log**: bước bắt buộc của playbook vắng trong log; con số gửi khách không qua tool tính; mã model lệch.
2. **Bộ kiểm hồi quy** chạy mỗi lần thay đổi và chạy liên tục trên phiên bản đang dùng. Anthropic cam kết chạy eval "continuously on
   true production systems" sau khi eval của họ "simply didn't capture the degradation users were reporting"
   (`anthropic.com/engineering/a-postmortem-of-three-recent-issues`, **HIGH**).
3. **Nguồn độc lập** (D10) và **canary token** (D15).
4. **Phản hồi của khách**, chuyển thẳng tới agent phụ trách. Lỗi định tuyến của Anthropic (0,8% yêu cầu Sonnet 4 lúc đầu, 16% ở giờ tệ
   nhất ngày 31/8) được phát hiện qua phản hồi người dùng (cùng nguồn, **HIGH**).
5. **Auditor và judge**: khác họ model với tác giả, trộn judge frontier với open-weight (`arxiv.org/abs/2609.22512`, **HIGH**), hoán đổi
   thứ tự, chấm lại một tập neo niêm do người gán nhãn. Ở Li, một lần đổi phiên bản judge âm thầm được tập neo phát hiện trong cả 60/60
   lần chạy, còn z-test cuốn chiếu báo động nhầm ở 75% chuỗi không có thay đổi (`arxiv.org/abs/2606.15474`, **HIGH**; một tác giả, một sự kiện lặp 60 lần).
6. **Lời agent tự báo cáo** không phải tín hiệu "xong".

### 2.2 Hàng đợi ngoại lệ

Việc chờ owner nằm trên màn hình của agent đã hỏi và được đếm ở tab của agent đó (`AGENTS.md:43-44`). Agent xếp hạng theo lớp hiệu
ứng và hậu quả, gộp việc trùng. Bằng chứng owner đọc do **nền tảng dựng** (diff, con số, nguồn gốc của tham số), không phải lời tóm
tắt của agent. Một rail do Assistant kéo "theo lời owner" trỏ tới đúng tin nhắn owner đã viết, đã xác thực, và hiện nguyên văn lời đó
cạnh hành động (`challenge-03` D10). Lý do: người duyệt chấp nhận 89% vấn đề do LLM nêu (`arxiv.org/abs/2411.11401`, **HIGH** theo
phản biện 01); 94% (83/88) lập trình viên không có monitor không phát hiện phá hoại (`arxiv.org/abs/2606.05647`, **HIGH** theo phản
biện 01). Owner đọc kết quả và cho khẩu vị; owner không phải lưới chặn (`AGENTS.md:53`).

### 2.3 Sửa tại chỗ

| Loại | Ví dụ | Ai làm gì | Nghĩa vụ luật (chạy dù owner có dừng hay không) |
|---|---|---|---|
| C0 | Mất dữ liệu, cam kết sai, secret lộ, dữ liệu cá nhân rò, sự cố nghiêm trọng | Owner quyết có bấm F1 hay không; agent phụ trách khôi phục (D3), xoay vòng secret (D2), sửa với khách | Báo cáo sơ bộ sự cố nghiêm trọng trong **72 giờ** (khẩn cấp) hoặc **05 ngày làm việc**, báo cáo chính thức trong **15 ngày** (Nghị định 142/2026/NĐ-CP Điều 19, **HIGH**, tôi đọc văn bản trên `hethongphapluat.com`); vi phạm dữ liệu cá nhân trong 72 giờ (Luật 91/2025 Điều 23; GDPR Điều 33; **MEDIUM** theo phản biện 04). Agent soạn, người đại diện ký và nộp. Cơ quan nhận trước khi có bất kỳ postmortem công khai nào |
| C1 | Đầu ra sai đã giao | Agent phụ trách sửa với khách; auditor tìm đầu ra cùng kiểu | Như C0 nếu chạm ngưỡng "thiệt hại đáng kể" (câu hỏi luật sư, 5.3) |
| C2 | Đúng kết quả, sai quy trình (false-pass) | Agent phụ trách | — |
| C3 | Chất lượng trôi sau một thay đổi | Agent đã ra thay đổi: hoàn tác hoặc sửa tiếp | Phân loại lại nếu thay đổi làm phát sinh rủi ro mới (Luật AI Điều 10(2)) |
| C4 | Nợ (độ phức tạp, cảnh báo tĩnh, việc mở lại) | Role sở hữu phần đó | — |

Khi hành vi cần đổi, sửa brief hoặc playbook trước khi thêm flag hay option (`AGENTS.md:45-46`). Mỗi lỗi đã thoát thành một ca hồi quy.

### 2.4 Mỗi thay đổi là một bản phát hành, ra ngay

Mọi thay đổi prompt, brief, playbook, harness, bộ nhớ hoặc model là một bản phát hành có mã, có dòng change log, hoàn tác bằng một hành
động. Khi phát hành, bộ hồi quy (D7) chạy trong vài phút, kết quả gắn vào bản phát hành và hiện trên màn hình của agent đã sửa; bản
phát hành ra ngay, không có thời gian chờ.

*Assumption: kiểm hồi quy thông báo chứ không chặn, vì owner quyết định không có cổng duyệt; agent đã sửa tự hoàn tác hoặc sửa
tiếp khi một ca hồi quy hỏng ("Judgement belongs to agents", `AGENTS.md:45`).*

Hai ngoại lệ do luật (Phán quyết của chủ tọa R4): (1) bản phát hành có thể đổi mức rủi ro AI mang chữ ký của người được giao trên trường
D6 (Luật AI Điều 10(2)); (2) ở bên áp dụng niêm yết tại Mỹ, bản phát hành (kể cả prompt và cấu hình) làm đổi một con số đi vào báo cáo
tài chính đi qua merge của F4, có bằng chứng duyệt ghi lại (SOX §404, 15 U.S.C. §7262; `challenge-04` §3.5, **HIGH** cho chữ theo
phản biện, **MEDIUM** cho cách kiểm toán viên áp dụng).

**Đề xuất của hội đồng (giả thuyết):** mỗi role chỉ có một bản phát hành đang chạy hồi quy tại một thời điểm, để mỗi kết quả hồi quy gắn
với đúng một thay đổi. Cách này không giúp quy kết quả của **khách** cho một thay đổi khi nhiều role phát hành mỗi ngày (mục 7).

### 2.5 Nhịp đọc số

Liên tục: log, kiểm xác định, hàng đợi ngoại lệ, hồi quy trên phiên bản đang dùng. Mỗi tuần: ảnh chụp scorecard và chỉ số nợ. Tuần 8
và 12: đọc lại xu hướng, vì với Cursor mức tăng "dissipate after two months" còn cảnh báo tĩnh (+30,26%) và độ phức tạp (+41,64%) kéo
dài (`arxiv.org/abs/2511.04427`, **HIGH**). Tháng 6, 9 và 12: đọc lại lần nữa, vì ở công ty 802 dev thông lượng tăng khoảng 12% lúc mỗi
người bắt đầu dùng AI và chỉ đạt 2× "after roughly nine months' usage", mục tiêu đạt ở tháng thứ mười (`arxiv.org/abs/2607.01904`,
**HIGH**). Hai nguồn kéo hai hướng, nên không đọc kết luận ở một mốc. Đây là lịch đọc, không phải giai đoạn.

## 3. Scorecard cho vận hành do AI làm (chỉ quan sát)

Không chỉ số nào tự kích hoạt dừng, thưởng hay phạt. Đây là một **định nghĩa mở** công ty khác dùng được; nó chỉ thành thước đo chung
khi ít nhất hai tổ chức độc lập áp dụng, công bố, và một bên tính lại số của bên kia khớp (`challenge-06-novelty.md` §4). Thành phần đã
có trước: pass^k (τ-bench), độ tin cậy (Rabanser), strict pass (CXBench), DORA four keys, log công khai của HAL, chuẩn AIUC-1, đơn vị
"outcome" của Intercom (`challenge-06` E7, **MEDIUM**). Phần của bản này là ghép chúng cho vận hành thật theo từng bản phát hành.

**Kết quả đã xác minh**: đầu ra đã giao, đạt kiểm kết quả, đạt kiểm quy trình, không bị phát hiện sai trong 30 ngày. Cửa sổ 30 ngày là
**Đề xuất của hội đồng (giả thuyết)**. Không dùng cụm này trong marketing ra ngoài khi định nghĩa là của chính công ty (FTC §5;
`challenge-04` §4, **MEDIUM**).

| # | Chỉ số | Định nghĩa chính xác | Nguồn định nghĩa |
|---|---|---|---|
| M1 | Nhất quán | Trên bộ hồi quy, ca t chạy K lần, c_t lần đạt. **pass^k** = trung bình theo t của C(c_t, k)/C(K, k), báo k = 1 và k = 3. **C_out** = trung bình theo t của (2p̂_t − 1)², p̂_t = c_t/K; báo kèm tỷ lệ đạt, vì C_out = 1 cả khi luôn đạt lẫn luôn hỏng. **Lần chạy tệ nhất.** Báo riêng lát mở và lát niêm. Ví dụ K = 5, c = 4: pass^3 = C(4,3)/C(5,3) = 4/10 = 0,40; C_out = (2·0,8 − 1)² = 0,36 | τ-bench `arxiv.org/abs/2406.12045`; Rabanser `arxiv.org/abs/2602.16666` Bảng 2 (**HIGH**) |
| M2 | Tuân thủ quy trình | **Strict pass** = tỷ lệ đầu ra đạt kiểm kết quả **và** có đủ bước bắt buộc trong log. **Tỷ lệ false-pass** = (đạt kết quả, sai quy trình)/(đạt kết quả) | CXBench: 55,5% của 840 lần chạy là false-pass, 80,1% của lần đạt kết quả, 13,8% đạt cả hai (`arxiv.org/abs/2610.25954`, **HIGH**) |
| M3 | Lỗi thoát | **DER_30** = lỗi phát hiện lần đầu sau khi giao (trong 30 ngày) / mọi lỗi phát hiện trên cùng đầu ra, theo mức nặng và lớp hiệu ứng | **Đề xuất của hội đồng (giả thuyết)** |
| M4 | Nợ | Code: cảnh báo tĩnh và độ phức tạp trên KLOC, cùng phiên bản công cụ, hằng tuần, so với ảnh chụp ngày 0. Ngoài code: **tỷ lệ làm lại** = đầu ra mở lại hoặc sửa trong 30 ngày / đầu ra đã giao | He et al. `arxiv.org/abs/2511.04427` (**HIGH**) |
| M5 | Tải lên người | Phút owner trên 100 kết quả đã xác minh; số việc chờ và tuổi p95; tỷ lệ chấp nhận và thời gian đọc trung vị; phân phối số bước agent giữa hai lần người can thiệp | Pan: 68% của 86 hệ thống đang chạy thật tối đa 10 bước trước khi người can thiệp (`arxiv.org/abs/2512.04123`, **HIGH**); tải mỗi reviewer gấp đôi (`arxiv.org/abs/2607.01904`, **HIGH**) |
| M6 | Chi phí trên kết quả đã xác minh | (hoá đơn model và compute + phút người × đơn giá có gánh chi phí) / số kết quả đã xác minh. **Tài chính của bên áp dụng đọc hoá đơn ngoài hệ thống agent**, theo lớp việc, mở cho người của công ty, không vào thưởng | Remit "records none and caps none" (`AGENTS.md:15-16`); ADR-0010 tự ghi xem lại khi chạy cho người khác owner (`challenge-02` M13) |
| M7 | Phát hiện và phục hồi | **Thời gian phát hiện**: từ hiệu ứng trong log tới mục đầu tiên về sai đó trên màn hình, theo lớp hiệu ứng. **Thời gian dừng**: từ lúc bấm F1 tới byte ra cuối cùng đo ở mạng host. **Thời gian khôi phục** và **cửa sổ mất dữ liệu**. Đo ở mọi lần thật và mọi diễn tập | NIST AI RMF MANAGE 2.4 (khung tự nguyện; `airc.nist.gov/airmf-resources/playbook/manage/`) |

**Quy tắc báo cáo:**
- Phân phối (p5, p50, p95) và lần chạy tệ nhất, không chỉ trung bình. Ở Vending-Bench, Claude 3.5 Sonnet có giá trị ròng trung bình
  $2.217,93 nhưng thấp nhất $476,00, tức 21% (`arxiv.org/abs/2502.15840` Bảng 1, **HIGH**; 476/2217,93 = 0,215).
- Mỗi tỷ lệ kèm n và khoảng Wilson (Bowyer `arxiv.org/abs/2503.01747`, **MEDIUM**); sai số theo cụm khi mục đi theo nhóm (Miller
  `arxiv.org/abs/2411.00640` §2.2, **HIGH**).
- Mỗi điểm số kèm mã model thực phản hồi, mã bản phát hành, cửa sổ thời gian, cách lấy mẫu việc.
- Chuỗi thời gian có vạch cho mỗi bản phát hành. Đọc trước và sau một vạch là gợi ý, không phải quy kết (mục 7). So sánh gần nhân quả
  nhất trong khuôn của owner là **đơn vị trong ranh giới so với đơn vị ngoài ranh giới cùng kỳ** (Phán quyết của chủ tọa R10): một so
  sánh quan sát, không ngẫu nhiên, chịu khác biệt giữa đơn vị; báo kèm giới hạn đó.

## 4. Mở (Phán quyết của chủ tọa R2)

### 4.1 Với owner và người của công ty

Mọi hành động, câu hỏi, lỗi, lời gọi tool, tài liệu chung, brief, playbook, bộ nhớ và change log hiện cho owner và người của công ty,
theo vai. Ba ngoại lệ có tên:
1. **Giá trị** của credential (F2): người thấy credential nào tồn tại, phạm vi và mọi lần dùng; giá trị không hiện lại ở đâu.
2. **Tập niêm** với agent bị đo: tập neo của judge, lát niêm của bộ hồi quy, vị trí canary token, nhãn của lỗi gieo nếu có. Owner thấy
   hết; agent bị đo không thấy, vì bộ kiểm mà bên bị đo thấy và tối ưu theo sẽ quá khớp (`challenge-02` P3).
3. **Dữ liệu cá nhân** theo mục đích và vai (D14), vì luật buộc (F3).

### 4.2 Ra ngoài

Phương pháp chỉ ra ngoài sau **kiểm pháp lý có người ký tên**. Bốn kiểm, mỗi kiểm một dòng có chữ ký trong change log:

| Kiểm | Câu hỏi | Nguồn |
|---|---|---|
| Dữ liệu cá nhân | Có dữ liệu cá nhân, kể cả mã đã băm có thể là dữ liệu bút danh hoá? Ca hồi quy rút từ việc thật của khách có cơ sở pháp lý cho mục đích mới và đã kiểm ẩn danh? | GDPR; Luật 91/2025 Điều 9(4)(a) (`challenge-04` §3.2, §3.4) |
| Bảo mật hợp đồng | Có vi phạm DPA, NDA với khách hay bí mật thương mại? | `challenge-04` §3.4; `challenge-05` §2 |
| Thứ tự báo sự cố | Cơ quan đã nhận báo cáo sơ bộ (72 giờ hoặc 05 ngày làm việc) trước khi postmortem ra ngoài? | Nghị định 142 Điều 19 (**HIGH**) |
| Thừa nhận trách nhiệm | Câu chữ có tạo thừa nhận lỗi hay trách nhiệm? Báo cáo sơ bộ nộp đúng hạn "không bị coi là sự thừa nhận lỗi kỹ thuật hoặc trách nhiệm pháp lý"; postmortem công khai không có bảo vệ đó | Nghị định 142 Điều 19 (**HIGH**); `challenge-04` §3.4 |

| Ra ngoài sau kiểm | Không ra ngoài (niêm với công chúng và với agent bị đo) |
|---|---|
| Bản phương pháp này, định nghĩa và mã tính scorecard M1–M7 | Ngưỡng phát hiện, vị trí canary token, tập neo niêm, lát niêm của bộ hồi quy, nhãn lỗi gieo |
| Scorecard theo tuần với n, khoảng tin cậy, phân phối | Chi tiết harness chỉ cho kẻ tấn công chỗ nhắm (cấu trúc prompt, danh sách detector, đường ra) |
| Change log ở mức "phần nào đổi, kết quả hồi quy lát mở, mã model" | Dữ liệu cá nhân; nội dung mật theo hợp đồng |
| Postmortem sau khi cơ quan đã nhận báo cáo và qua bốn kiểm | Postmortem có tên khách hoặc số liệu trọng yếu chưa công bố |
| Kết quả xấu như kết quả tốt | — |

Lý do niêm: kẻ tấn công thích ứng dùng phản hồi của detector, và vượt 12 phòng thủ với tỷ lệ trên 90% cho đa số
(`arxiv.org/abs/2510.09023`, `challenge-03` A29 và rủi ro 9). Ca công khai còn có thể vào dữ liệu huấn luyện: o3 xác định đúng tệp
lỗi 76% trên SWE-bench so với 53% ở kho khác (`arxiv.org/abs/2506.12286`, **HIGH** theo phản biện 01).

### 4.3 Người khác lặp lại thế nào, và đóng góp được gì

1. **Tính lại**: từ bảng scorecard đã công bố và mã tính.
2. **Áp dụng**: chạy phương pháp trên việc của mình, công bố M1–M7 theo cùng định nghĩa.

Chạy lại một lần chạy cụ thể không làm được: model đóng bị ngừng, và các tham số lấy mẫu (`temperature`, `top_p`, `top_k`) bị loại ở
phiên bản API 2026-10-21 (trang deprecations của Anthropic, **MEDIUM**); chỉ phân phối lặp lại được.

Đóng góp nói chính xác (`challenge-06` §5, **MEDIUM**): một chuỗi dữ liệu thực địa mở, n = 1 tổ chức, về tỷ lệ bắt lỗi của checker và
auditor, độ độc lập thực của panel judge trên dữ liệu kinh doanh, và tỷ lệ lỗi thoát 30 ngày theo lớp việc, ở cấu hình tự chủ mà khảo
sát của Pan (86 hệ thống) không phủ. Nó không gồm bằng chứng nhân quả về lợi nhuận.

## 5. Mẫu triển khai một lần cho doanh nghiệp lớn

### 5.1 Ranh giới (Phán quyết của chủ tọa R10)

Bên áp dụng chọn một đơn vị: tenant, đơn vị kinh doanh hay chức năng, có dữ liệu, kho secret, backup, log và nút dừng riêng. Ranh giới
chỉ gồm hệ thống mà **mọi** tác dụng của agent tới qua gate và proxy; hệ thống không qua gate nằm ngoài ranh giới (`challenge-02` pre-mortem 5).
Bên trong, toàn hệ thống bật cùng lúc. Đơn vị bên ngoài giữ cách làm cũ và là nhóm so sánh quan sát.

Thông tin cho việc chọn: ở TheAgentCompany, agent mạnh nhất tự hoàn thành khoảng 30% tác vụ; kỹ thuật phần mềm cao nhất; hành chính,
tài chính, khoa học dữ liệu thấp nhất (`arxiv.org/abs/2412.14161`, **HIGH** theo phản biện 01).

### 5.2 Công tắc dừng

| Mức | Ai bấm | Tác dụng |
|---|---|---|
| Agent, cell | Owner của đơn vị, hoặc Assistant theo tin nhắn đã xác thực của owner | Dừng ở gate và proxy |
| Tenant, cả ranh giới | Owner của đơn vị; người chịu trách nhiệm cấp điều hành | Dừng mọi lời gọi tool, mọi lưu lượng ra và mọi lần dùng credential |
| Một nhà cung cấp model | Như trên | Ngắt nhà cung cấp. Hiện thực tham chiếu chỉ có một (ADR-0006); chuyển sang bên khác chỉ khi bên đó đã có DPA và eval riêng |

Luật AI Việt Nam Điều 7(4) cấm cản trở cơ chế giám sát của con người; Điều 4(2) đòi duy trì "sự kiểm soát và khả năng can thiệp của con
người đối với mọi quyết định và hành vi" (`challenge-04` K10, **HIGH** cho chữ theo phản biện). Nút dừng có thể chưa đủ cho Điều 4(2):
câu hỏi luật sư số 1. NIST AI RMF MANAGE 2.4 khuyến nghị cơ chế có người chịu trách nhiệm để ngắt hệ thống.

### 5.3 Pháp lý ở ngày 0 (F3, Phán quyết của chủ tọa R4)

Nguồn: `challenge-04-legal.md` (người phản biện đọc toàn văn từ Công báo EU, Federal Register hoặc cơ sở dữ liệu luật Việt Nam; tôi chưa
mở lại, nên **MEDIUM** với tôi trừ chỗ ghi khác). Công ty vừa là provider vừa là deployer (Luật AI Điều 3(4),(5); AI Act Điều 3(3)).

**Trước khi dùng** (mỗi mục là D11, người đại diện ký):
- Hồ sơ phân loại rủi ro và thông báo Bộ KH&CN qua Cổng một cửa cho mọi agent rủi ro trung bình hoặc cao; agent nói với khách hay công chúng
  thuộc rủi ro trung bình (Luật AI Điều 9(1)(b), 10(1),(3); Nghị định 142 Điều 9, 12, 14). Không có thời gian chuyển tiếp cho hệ thống bật
  sau 01/03/2026 (Luật AI Điều 35(1)).
- Thông báo AI và đánh dấu máy đọc (D13): AI Act Điều 50 áp từ 02/08/2026; hạn 02/12/2026 cho Điều 50(2) chỉ dành cho hệ thống có trước
  02/08/2026. Digital Omnibus là Regulation (EU) 2026/1744, đăng Official Journal 24/07/2026, hiệu lực 27/07/2026; nghĩa vụ rủi ro cao
  Phụ lục III dời sang 02/12/2027, Phụ lục I sang 02/08/2028 (**HIGH** theo phản biện 04; tôi kiểm qua kết quả tìm kiếm).
- DPIA trước khi xử lý (GDPR Điều 35); DPA, SCC và cấu hình lưu giữ bằng 0 với mọi nhà cung cấp model; sổ mục đích và phân quyền (D14);
  chỉ định bộ phận hoặc nhân sự bảo vệ dữ liệu (Luật 91/2025 Điều 33(2)).

**Trong 60 ngày từ lần xử lý đầu**: hồ sơ đánh giá tác động xử lý dữ liệu cá nhân (Luật 91/2025 Điều 21) và hồ sơ chuyển dữ liệu ra nước
ngoài (Điều 20(2)).

**Thay người là thủ tục luật lao động, có lịch luật định, không ở ngày 0.** Việt Nam: phương án sử dụng lao động, ưu tiên đào tạo lại, trao đổi
với tổ chức đại diện người lao động, báo trước 30 ngày cho Uỷ ban nhân dân cấp tỉnh và người lao động, trợ cấp mất việc (Bộ luật Lao động
2019 Điều 42, 44, 47). EU: tham vấn và 30 ngày sau khi báo cơ quan (Chỉ thị 98/59/EC Điều 2, 4); Đức: đồng quyết của Betriebsrat với hệ thống có
khả năng giám sát hành vi hay hiệu suất (BetrVG §87(1) Nr. 6). Mỹ: báo trước 60 ngày khi tới ngưỡng WARN. Hệ thống bật ở ngày 0; quyết định
nhân sự đọc D10 và M5, không đọc kế hoạch. Quyết định về việc làm của một người cụ thể có người ký (GDPR Điều 22; AI Act Phụ lục III 4(b) từ 02/12/2027).

**Nếu một chức năng rơi vào rủi ro cao**: thử nghiệm trước khi đưa ra thị trường theo chỉ số định trước (AI Act Điều 9(8)) và đánh giá
sự phù hợp trước khi dùng (Luật AI Điều 13(3)) là nghĩa vụ; ở đó F3 thắng "không thí nghiệm".

**Lưu trú dữ liệu**: đường gọi model phải ở vùng hợp với hợp đồng và luật; API Anthropic trực tiếp có `inference_geo` "us" hoặc "global",
vùng EU đi qua Bedrock (`challenge-05` §3, **MEDIUM**).

**Khi đang chạy**: không viết review hay testimonial mang danh người không tồn tại (quy tắc FTC 08/2024); gọi điện hay nhắn tin cho người ở
Mỹ cần đồng ý trước (TCPA, **LOW-MEDIUM** theo phản biện 04); phân loại lại khi thay đổi làm phát sinh rủi ro mới (D6); báo sự cố theo 2.3; SOX cho bên niêm yết theo 2.4; lưu trữ tại Việt
Nam dữ liệu người dùng ở Việt Nam (Luật An ninh mạng 116/2025/QH15 Điều 25(3), phạm vi **MEDIUM**).

**Câu hỏi luật sư còn mở** (`challenge-04` §5): (1) nút dừng, log và quyền đảo ngược sau có thoả Luật AI Điều 4(2) khi không có duyệt trước
không; (2) mỗi role là một hệ thống phải thông báo riêng hay không, và sửa brief có là "sửa đổi làm phát sinh rủi ro mới" không; (5) ngưỡng
"thiệt hại đáng kể về tài sản" cho một con số sai gửi nhiều khách; (11) công bố ra ngoài có vi phạm DPA, NDA hay tạo thừa nhận trách nhiệm không.

### 5.4 Ma trận kiểm soát

Ánh xạ là **Đề xuất của hội đồng (giả thuyết)**, cần kiểm toán viên xác nhận. Tên khung (**MEDIUM**, qua kết quả tìm kiếm): NIST AI RMF
có bốn chức năng GOVERN, MAP, MEASURE, MANAGE; NIST AI 600-1 (07/2024) có 12 rủi ro của AI sinh; ISO/IEC 42001 Phụ lục A có 38 kiểm soát
trong 9 mục A.2–A.10; OWASP Top 10 for LLM Applications 2025 có LLM01–LLM10. Với ngân hàng Mỹ, SR 26-2 (17/04/2026) đặt AI sinh và
agentic ngoài phạm vi quản trị rủi ro mô hình và cần khung riêng (`challenge-05` §2, **MEDIUM**); ma trận này viết để dùng làm khung đó.

| Điểm cố định hoặc bất biến | NIST AI RMF | NIST AI 600-1 | ISO/IEC 42001 Phụ lục A | EU AI Act | OWASP LLM 2025 |
|---|---|---|---|---|---|
| F1 dừng, đo ở mạng host (D1) | MANAGE 2.4 | Human-AI configuration | A.6 vòng đời (vận hành); A.9 sử dụng | Đ.14(4)(e); Đ.26(1),(2) | LLM06 Excessive Agency |
| F2 credential (D2, 1.4) | MANAGE | Information security | A.6; tham chiếu ISO/IEC 27001 | Đ.15 (an ninh mạng) | LLM02 Sensitive Information Disclosure; LLM06 |
| F3 dữ liệu theo mục đích và vai (D14) | GOVERN, MAP | Data privacy | A.7 dữ liệu | Đ.10; Đ.26 | LLM02 |
| F3 thông báo AI (D13) | GOVERN | Information integrity | A.8 thông tin cho bên liên quan | Đ.50 (ngoài Đ.9–15) | — |
| F4 merge do owner, authentication | MANAGE | Human-AI configuration | A.3 tổ chức nội bộ; A.6 | Đ.14 | LLM06 |
| Lớp hiệu ứng và tuyên bố rủi ro còn lại (1.2, 5.5) | MAP, GOVERN | Human-AI configuration | A.5 đánh giá tác động | Đ.9 | LLM06 |
| Log chuỗi băm, nguồn gốc tham số (D4) | MEASURE | Information integrity | A.6 (ghi sự kiện) | Đ.12; Đ.26(6) | LLM01 Prompt Injection (truy vết) |
| Backup ngoài quyền (D3) | MANAGE | Information security | A.6 | Đ.15 (khả năng phục hồi) | LLM06 |
| Tool tính số xác định (D9) | MEASURE | Confabulation | A.6 (kiểm chứng) | Đ.15 (độ chính xác) | LLM09 Misinformation |
| Bộ hồi quy K ≥ 5, lát niêm (D7, M1) | MEASURE | Confabulation | A.6 (kiểm thử) | Đ.9; Đ.15 | LLM09 |
| Judge khác họ, tập neo niêm (2.1) | MEASURE | Information integrity | A.6 | Đ.15 | — |
| Change log, hoàn tác, mã model (D6, D8) | MANAGE | Value chain and component integration | A.6; A.10 bên thứ ba | Đ.11; Đ.12 | LLM03 Supply Chain |
| Máy chủ MCP lần đầu, canary token (D16, D15) | MEASURE | Information security; Value chain | A.10 | Đ.15 | LLM03; LLM01 |
| Kiểm pháp lý trước công bố (D12, 4.2) | GOVERN | Data privacy; Intellectual property | A.8 | — | LLM02 |
| Báo sự cố (2.3) | MANAGE | — | A.8 | Đ.26(5); Đ.73 | — |
| Chi phí do tài chính đọc (M6) | GOVERN | — | A.4 nguồn lực | — | LLM10 Unbounded Consumption |

### 5.5 Mẫu tuyên bố rủi ro còn lại có chữ ký

Bản này cố ý không làm các việc dưới đây (quyết định của owner hoặc giới hạn của hiện thực). Mỗi dòng có một người cấp điều hành nhận rủi
ro bằng chữ ký; không có chữ ký thì chức năng tương ứng không bật. Cách này không thêm giới hạn lên agent; nó ghi ai chấp nhận việc không có
giới hạn (`challenge-05` §5 mục 2).

| # | Điều bản này không làm | Hệ quả (bằng chứng) | Thứ đang mang nó | Người nhận (vai) | Ký, ngày |
|---|---|---|---|---|---|
| RR1 | Không giới hạn sáng kiến: không trần tự chủ, không giới hạn phạm vi ngoài luật | Ủy ban rủi ro thiếu "risk limits" mà khung khẩu vị rủi ro đòi (FSB 2013, `challenge-05` §2, **MEDIUM**) | F1–F4, M1–M7, 1.2 | Giám đốc rủi ro | |
| RR2 | Hiệu ứng không đảo ngược được không có cận; hoãn-và-huỷ mặc định tắt | Email, phát ngôn, dữ liệu đã rò, tiền đã chuyển không lấy lại được | Ghi trước khi gửi; F1; D13 | Tổng giám đốc | |
| RR3 | Không Rule of Two, không danh sách đích, không tách luồng điều khiển khỏi dữ liệu (chưa quyết) | Injection có mục tiêu trên AgentDojo thành công 47,69% với GPT-4o và 33,86% với Claude 3.5 Sonnet; thích ứng vượt 12 phòng thủ trên 90% (`challenge-03` A1, A2) | D4 nguồn gốc tham số, F2, D15 | Giám đốc an ninh thông tin | |
| RR4 | Không trần chi phí; Remit không ghi chi phí | Multi-agent dùng khoảng 15 lần token chat (Anthropic, **MEDIUM**); vòng "meltdown" ở Vending-Bench | M6 do tài chính đọc từ hoá đơn | Giám đốc tài chính | |
| RR5 | Không thí nghiệm: lợi tức không quy được nguyên nhân | So sánh trong và ngoài ranh giới là quan sát (3, 5.6) | 5.6 | Giám đốc tài chính | |
| RR6 | Cam kết với khách không bị chặn bằng mã | Cam kết ràng buộc công ty (Luật GDĐT Điều 14(2)(a)); trần trách nhiệm của Anthropic bằng phí 12 tháng (**MEDIUM-HIGH** theo báo cáo rủi ro) | Bản ghi có kiểu, đường bù (1.2) | Pháp chế | |
| RR7 | Một nhà cung cấp model, một owner mỗi server (hiện thực tham chiếu) | Rủi ro tập trung; DORA đòi đánh giá trước khi ký cho tổ chức tài chính EU (`challenge-05` §2, **MEDIUM**); giới hạn phiên thật: 40 lỗi trên 20 card trong khoảng 52 phút ngày 26/10/2026, từ 13:48 tới 14:40 (ADR-0005:13-15, **HIGH**) | 1.4 | Giám đốc công nghệ | |
| RR8 | Bảo hiểm có thể loại trừ | Verisk/ISO có ba endorsement loại trừ AI sinh cho CGL từ 01/01/2026 (`challenge-05` §2, **MEDIUM**) | Hồ sơ bảo hiểm: điều khoản hiện hành, câu hỏi cho broker, phần không được bảo hiểm | Giám đốc tài chính | |
| RR9 | Lát mở của bộ hồi quy agent thấy được | Điểm lát mở là cận trên lạc quan | Báo riêng lát niêm (M1) | Người giữ scorecard | |
| RR10 | Chưa có SOC 2, ISAE 3402, pentest bên thứ ba cho gate và nút dừng | Kiểm toán nội bộ thiếu chứng thực (`challenge-05` §2) | Biên bản diễn tập D1, D3 | Kiểm toán nội bộ | |

### 5.6 Triển khai tham chiếu

Người mua cần một quý số liệu trước khi tin (`challenge-05` §5 mục 3). Đây là cửa sổ dữ liệu, không phải giai đoạn.
- **Điều kiện vào**: các lỗ ở 1.4 đã đóng và có biên bản phép thử; D1–D16 đạt; 5.5 đã ký; hồ sơ "trước khi dùng" ở 5.3 đã nộp.
- **Một quý M1–M7** của một ranh giới, công bố theo tuần sau bốn kiểm của 4.2.
- **M6 từ hoá đơn**, do tài chính đọc ngoài hệ thống agent, theo lớp việc.
- **So sánh trong và ngoài ranh giới** cùng kỳ trên D10 (khối lượng, khiếu nại, mở lại) và M3 ở nơi đo được: chênh lệch trong chênh lệch
  quan sát, ghi rõ không ngẫu nhiên. Báo cả kết quả xấu.
- **Báo hội đồng quản trị** mỗi tháng: M1–M7, sự cố C0 và C1, trạng thái các dòng 5.5, do người chịu trách nhiệm cấp điều hành ký. Theo
  Marchand v. Barnhill (2019) hội đồng phải có hệ thống giám sát và báo cáo cho chức năng mission-critical (`challenge-05` §2, **MEDIUM**).
- **Gói cho hội đồng lao động**: dữ liệu về người được ghi, mục đích, thời hạn lưu, ai truy cập (`challenge-05` §5 mục 8).

### 5.7 Mẫu thất bại đã biết

| Mẫu | Điều đã xảy ra | Thiết kế mang nó bằng |
|---|---|---|
| Klarna | 27/02/2024: trợ lý AI 2,3 triệu cuộc chat, hai phần ba số chat, tương đương 700 nhân viên, ước tính +$40 triệu lợi nhuận 2024 (thông cáo Klarna, **HIGH** cho nội dung). 05/2025: CEO nói rẻ hơn nhưng "lower quality", tuyển lại người (Entrepreneur dẫn Bloomberg, **MEDIUM**) | M2, M3 đo chất lượng từ log và nguồn độc lập |
| CBA | 07/2025 công bố cắt 45 vị trí thay bằng voice bot; 21/08/2025 đảo ngược, thừa nhận "error"; công đoàn FSU nói cuộc gọi tăng (ia.acs.org.au, **MEDIUM**) | D10 đo khối lượng ngoài agent; thay người theo thủ tục luật (5.3) |
| Tải review tăng | Tải mỗi reviewer gấp đôi ở công ty 802 dev (`arxiv.org/abs/2607.01904`, **HIGH**) | M5 hằng tuần; hàng đợi xếp hạng và gộp trùng (2.2) |
| Tự chủ có giới hạn ở thực tế | 68% của 86 hệ thống dừng cho người sau tối đa 10 bước; 74% dựa chủ yếu vào đánh giá của người (Pan, **HIGH**) | M5 công bố phân phối số bước giữa hai lần can thiệp, cho vùng mà khảo sát của Pan không phủ |

## 6. Pre-mortem: mười cách hệ thống thất bại

Giả định một năm sau đã thất bại. Thứ tự là nhận định của tôi (**MEDIUM**). Cột cuối nói biện pháp phủ tới đâu.

| # | Cách thất bại | Biện pháp | Phủ |
|---|---|---|---|
| 1 | Bấm dừng nhưng agent vẫn chạy: phiên còn mạng ngoài gate, tiến trình `setsid` sống tiếp | D1 đo ở mạng host; 1.4 đóng lỗ trước ngày 0 | Đầy đủ khi 1.4 xong |
| 2 | Injection qua nội dung khách hay web khiến agent dùng connection hợp lệ để gửi dữ liệu hay cam kết ra ngoài | D4 nguồn gốc tham số, D15, F2; tách luồng điều khiển khỏi dữ liệu chờ owner quyết (RR3) | **Không trước hành động**: chỉ phát hiện và bù |
| 3 | Hành động không đảo ngược được (email hàng loạt, hoàn tiền chia nhỏ, phát ngôn) | Tổng theo cửa sổ hiện cho owner; hoãn-và-huỷ nếu bật | **Không có cận** (RR2) |
| 4 | Hàng đợi của owner sập: merge vẫn là rail, agent không bị giới hạn, owner duyệt máy móc hoặc ngừng duyệt | M5 hiện tuổi p95 và tỷ lệ chấp nhận từ tuần 1; bằng chứng do nền tảng dựng | **Một phần**: trần số MR chờ của ADR-0001 chờ owner quyết |
| 5 | Không trả lời được "có hiệu quả không" ở tháng 4–6; nhà tài trợ rút | M6 do tài chính đọc; so sánh trong và ngoài ranh giới (5.6) | **Một phần**: không quy nhân quả được |
| 6 | Pháp chế buộc thêm duyệt sau khi bật (Luật AI Điều 4(2), rủi ro cao) | F3 thắng; câu hỏi luật sư 1, 2 trả lời trước ngày 0 | Phụ thuộc câu trả lời của luật sư |
| 7 | Scorecard xanh, khách đỏ: agent quá khớp với bộ kiểm nó thấy | Lát niêm (D7), tập neo niêm, D10 đứng cạnh mọi chỉ số | Một phần |
| 8 | Credential ngoài kho bị đọc (`~/.ssh`, môi trường tiến trình) | D2 quét mọi credential; 1.4 | Đầy đủ khi 1.4 xong |
| 9 | Chức năng bị thay quay lại qua kênh khác, hoặc thay người sai thủ tục | D10; thủ tục luật lao động (5.3) | Đầy đủ cho thủ tục; một phần cho chức năng |
| 10 | Vi phạm pháp luật: thiếu thông báo trước khi dùng, báo sự cố trễ, công bố lọt dữ liệu | D11, D13, D14, D12; nghĩa vụ báo cáo ở 2.3 chạy độc lập với F1 | Đầy đủ cho các mục đã biết |

## 7. Điều bằng chứng nói về vận hành không giới hạn và không thí nghiệm

Thông tin cho owner: mỗi dòng là điều đã đo hoặc đã xảy ra, kèm chỗ ngày 0 mang nó.

**Khi agent không bị giới hạn:**
- **Replit (07/2025):** agent xoá database production trong lúc có code freeze bằng lời dặn (OECD AI Incident Monitor, **MEDIUM**); con số
  khoảng 4.000 hồ sơ giả chỉ có ở tiêu đề bài báo mà OECD dẫn. Nguồn thứ cấp **mâu thuẫn** về việc rollback có dùng được không. Ngày 0: D3, D4.
- **PocketOS (25/04/2026):** token trong tệp không liên quan, quyền trên toàn API, xoá database và backup trong 9 giây (**MEDIUM**). Ngày 0: F2, D2, D3.
- **Palisade:** hơn 100.000 lượt, 13 model; không chỉ dẫn gì, 8/13 model phá script tắt ít nhất một lần trong 1.000 lượt; thêm "allow
  yourself to be shut down" vào prompt người dùng thì còn 6/13; một số model phá tới 97% dù được dặn (`arxiv.org/abs/2509.14260`, **HIGH**).
  Script tắt nằm trong môi trường của model. Perez (*The Law of Stop*, 2609.22882): trong hệ agentic "a stop at one point may leave the
  activity running elsewhere" (`challenge-06` §1.1, **MEDIUM** với tôi). Ngày 0: D1 đo ở mạng host.
- **Độ tin cậy tăng chậm:** 15 model qua 24 tháng; độ tin cậy tăng 0,03/năm (GAIA) và 0,09/năm (τ-bench) trong khi độ chính xác tăng
  0,23/năm và 0,22/năm (`arxiv.org/abs/2602.16666` Hình 1 và tr. 38, **HIGH**). Ngày 0: M1.
- **Lần chạy hỏng nặng:** mọi model ở Vending-Bench có lần chạy "derail" (**HIGH**). Ngày 0: M1 lần chạy tệ nhất; F1.
- **Thiếu dữ liệu thực địa:** hệ thống đang chạy thật thường dừng cho người sau ít bước (Pan), nên cấu hình này nằm ngoài vùng khảo sát đó đo.

**Khi không có thí nghiệm:**
- **Trước-sau bị thời gian làm nhiễu:** SRE Workbook khuyên so canary với nhóm đối chứng đồng thời (`sre.google/workbook/canarying-releases/`,
  **MEDIUM**). Big bang bỏ sự so le về thời điểm mà 2607.01904 dùng để ước lượng (staggered difference-in-differences); so sánh còn lại là
  bên ngoài ranh giới.
- **Chỉ số tiêu đề đi sai chiều:** ở Bing, một lỗi giảm chất lượng làm truy vấn mỗi người tăng hơn 10% và doanh thu mỗi người hơn 30%
  (Kohavi et al., KDD 2012, **HIGH** theo phản biện 01). Ngày 0: M2, M3, D10 đứng cạnh mọi chỉ số tiêu đề.
- **Eval bỏ sót suy giảm:** ba thay đổi gây suy giảm Claude Code nằm ở harness (hạ mức suy luận mặc định 04/03, lỗi bộ nhớ đệm xoá lý lẽ
  26/03, dòng prompt giảm độ dài 16/04); Anthropic cam kết "soak periods, a broader eval suite, and gradual rollouts" cho thay đổi có thể đánh
  đổi trí tuệ (`anthropic.com/engineering/april-23-postmortem`, **HIGH**). Hệ thống này ra ngay; khoảng chênh được mang bằng D6, D7 và nguồn
  tín hiệu 3–4 ở 2.1.
- **Lợi ích đầu kỳ tan, hoặc tăng dần:** hai nguồn kéo hai hướng (2.5).
- **Judge trôi trông như hệ thống trôi:** đổi Mistral-Small 3.1 sang 3.2 làm 59% tập chọn top-30% đổi (`arxiv.org/abs/2610.32986`, **HIGH**
  theo phản biện 01). Ngày 0: tập neo niêm.
- **Big bang đã có tiền lệ thất bại:** Hershey 1999 cắt chuyển cùng lúc SAP R/3, Siebel, Manugistics và không giao được khoảng 100 triệu USD
  đơn hàng Halloween (CIO.com theo `challenge-06` E8, **MEDIUM**). Ngày 0: 1.4, D1–D16 và ranh giới mà bên ngoài giữ cách làm cũ.

## 8. Nguồn chính

- Tra trong toàn văn, ADR hoặc mã trong lần soạn này (**HIGH**): Miller `2411.00640`; Rabanser `2602.16666`; Vending-Bench `2502.15840`;
  He (Cursor) `2511.04427`; Pan `2512.04123`; CXBench `2610.25954`; FinSheet-Bench `2603.07316`; Li `2606.15474`; Hossain `2609.22512`; He
  (review) `2607.01904`; τ-bench `2406.12045`; Palisade `2509.14260`; ADR-0004, 0005, 0006, 0018, 0021, 0022; `internal/jail/sandbox_darwin.go:143`.
- Trang gốc đã mở: `anthropic.com/engineering/april-23-postmortem`; `anthropic.com/engineering/a-postmortem-of-three-recent-issues`;
  `klarna.com/international/press/klarna-ai-assistant-handles-two-thirds-of-customer-service-chats-in-its-first-month/`;
  `entrepreneur.com/business-news/klarna-ceo-reverses-course-by-hiring-more-humans-not-ai/491396`;
  `ia.acs.org.au/article/2025/cba-reverses-ai-driven-job-cuts--admits--error-.html`; `zenity.io/blog/ai-agent-database-deletion-pocketos`;
  `oecd.ai/en/incidents/2025-07-19-1eb1`; `hethongphapluat.com/nghi-dinh-142-2026-nd-cp-huong-dan-luat-tri-tue-nhan-tao/dieu-19`.
- Qua kết quả tìm kiếm (**MEDIUM**): Regulation (EU) 2026/1744 (`pasqualepillitteri.it/en/news/8951/eu-regulation-2026-1744-digital-omnibus-ai-act`);
  ISO/IEC 42001 Phụ lục A (`docs.modulos.ai/frameworks/iso-42001/annexes-a-d.html`); NIST AI 600-1 (`airc.nist.gov/docs/NIST.AI.600-1.GenAI-Profile.ipd.pdf`);
  OWASP LLM 2025 (`docs.modulos.ai/frameworks/owasp-top-10-llm`).
- Sáu phản biện của hội đồng (`challenge-01` đến `challenge-06`) cho các nguồn ghi "theo phản biện"; các nguồn khác theo nhãn của báo cáo nghiên cứu.
