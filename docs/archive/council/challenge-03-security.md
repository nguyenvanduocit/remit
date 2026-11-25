# Phản biện 03: Red team bảo mật

> Thành viên red team bảo mật của hội đồng, phản biện `02-design.md` và `04-launch.md` (kèm `01-strategy.md`, `03-ledger.md`).
> Ngày 2026-11-01. Không sửa bản nháp. Con số từ PDF trong `docs/papers/` được tôi trích bằng pypdf trong venv tạm ở `/tmp` (đã xoá)
> và ghi kèm số dòng của văn bản trích (`2503.18813:725` = dòng 725). Nhãn: **HIGH** (đã đọc nguồn gốc trong lần soạn này),
> **MEDIUM** (suy ra từ nguồn), **LOW** (phỏng đoán, cần kiểm).
>
> Phạm vi: chỉ đọc tài liệu, ADR và mã nguồn; tôi không thăm dò máy đang chạy. Tài liệu này nêu đường tấn công và biện pháp ở mức
> thiết kế, không ghi các bước khai thác.

## 0. Kết luận trong năm dòng

1. Bản thiết kế coi một **gate** duy nhất là ranh giới, nhưng hiện thực tham chiếu mà nó dẫn (ADR-0018, ADR-0021) cho phiên agent
   **mạng trực tiếp và quyền như user của server**, ngoài gate. Vì vậy I1, F1 và một phần F3 là lời hứa, chưa là thuộc tính. **HIGH**.
2. Bản thiết kế xếp việc tách luồng điều khiển khỏi dữ liệu (CaMeL) vào loại "giới hạn". Nhưng phần lớn hiệu quả của CaMeL đến
   **không cần chính sách nào**: "we install no security policy and all security benefits come from explicit separation"
   (`2503.18813:724-726`). Biện pháp mạnh nhất có đo đạc đang bị bỏ vì xếp nhầm loại. **HIGH**.
3. Sổ ghi và hiển thị taint, cách duy nhất bản thiết kế dùng với injection, có hiệu quả đo được gần bằng không khi đứng riêng:
   gắn nhãn nguồn chỉ giảm ASR 5% (`2410.07283:358`). **HIGH**.
4. Hành động không đảo ngược (email gửi khách, hoàn tiền, phát ngôn công khai, dữ liệu gửi tới nhà cung cấp) không có biện pháp
   nào trước khi xảy ra, và backup không trả lại được chúng. Bản thiết kế thừa nhận điều này (`02-design.md:432-433`); bản này đề
   xuất cách giảm hại mà **không** đặt cổng duyệt.
5. Phần lớn sửa đổi dưới đây là **sự thật của nền tảng** (thuộc tính của kênh, không phải quyết định từ chối hành động), nên không
   thuộc định nghĩa "giới hạn" của owner. Mỗi đề xuất ghi rõ nó có phải giới hạn hay không và vì sao.

## 1. Định nghĩa dùng để phân loại

Quyết định 2 của owner cấm: mức tự chủ, chứng chỉ theo role, trần chi phí, giới hạn phạm vi, cổng phê duyệt, dừng tự động theo
hiệu năng (`02-design.md:19-20`). Bản này dùng phép thử sau: **một biện pháp là giới hạn nếu nó khiến một hành động mà agent đã
chọn không xảy ra, hoặc phải chờ một chủ thể khác**. Một biện pháp chỉ đổi *cách* hành động đi qua (kênh, bằng chứng, nhãn, khả
năng hoàn tác), còn mọi hành động vẫn xảy ra, thì không là giới hạn. Ví dụ: ghi log, ghi nhãn, ghim phiên bản, hoãn gửi có hủy
được (agent vẫn gửi; nền tảng giữ một cửa sổ để rút lại).

## 2. Cây tấn công theo ranh giới tin cậy

Ranh giới B1–B7 lấy từ `research-ai-autonomous-system-risk.md:169-170`. Khả thi: **Cao** (đã đo ≥ 30% trong môi trường gần giống,
hoặc đường đi đã có trong mã), **TB** (đo 5–30%, hoặc cần điều kiện thêm), **Thấp** (< 5% hoặc chỉ trong sandbox nhân tạo). Tầm
hại dưới "không giới hạn": **Lớn** (không đảo ngược, ra khỏi công ty, hoặc tắt khả năng thấy), **Vừa** (đảo ngược được, tốn kém),
**Nhỏ**.

### B1. Nội dung ngoài → agent

| # | Kịch bản | Khả thi (bằng chứng) | Tầm hại |
|---|---|---|---|
| A1 | Email hoặc ticket của khách chứa chỉ thị; agent hỗ trợ khách làm theo (gửi thông tin, đổi đơn, hoàn tiền) | **Cao**: ASR có mục tiêu 47,69% (GPT-4o), 33,86% (Claude 3.5 Sonnet) (`2406.13352:944-950`); injection gián tiếp 27,1% so với trực tiếp 5,7% (`2507.20526:208`) | **Lớn**: role vừa đọc nội dung ngoài vừa có mọi connection (`02-design.md:195`) |
| A2 | Kẻ tấn công kiên trì thử lặp tới khi qua | **Cao**: vi phạm trong 10–100 truy vấn (`2507.20526:19`); thích ứng vượt 12 phòng thủ > 90% (`2510.09023:23`) | Lớn |
| A3 | Trang web đọc khi nghiên cứu khiến hệ nhiều agent chạy mã | **Cao**: 58–90% (`2503.12188:18`) | **Lớn**: mã chạy trong phiên có mạng trực tiếp (mục 3.1) |
| A4 | Injection "chờ": nằm im trong bộ nhớ dài hạn tới khi agent có công cụ cần thiết | **TB**: AgentDojo nêu đây là chỗ lọc công cụ thất bại (`2406.13352:496-499`) | Lớn |
| A5 | Đầu độc bộ nhớ chung: một ghi chú "kinh nghiệm" khiến về sau agent hoàn tiền dễ hơn | **TB**: agent tự tiến hoá bộ nhớ học tương quan sai giữa hoàn tiền và phản hồi tốt (`2509.26354:144-145`) | Vừa, cộng dồn |
| A6 | Nội dung ngoài đổi **tham số** của một hành động hợp lệ (người nhận, số tiền, URL) | **Cao**: dạng hiệu quả nhất của MCPTox, ASR trung bình 46,7% (`2508.14925:592-597`) | Lớn: hành động trông đúng với ý định |

### B2. Agent ↔ agent và tài liệu chung

| # | Kịch bản | Khả thi | Tầm hại |
|---|---|---|---|
| A7 | Chair không đọc nội dung ngoài vẫn bị chiếm qua kết quả của sub-agent | **Cao**: orchestrator không chạm nội dung ngoài vẫn bị chiếm (`2503.12188:279-282`) | **Lớn**: Chair điều phối mọi role |
| A8 | Injection tự nhân bản qua tin nhắn và mục ngữ cảnh chung | **TB–Cao**: GPT-4o bỏ qua 66% (`2410.07283:248`); model mạnh hơn làm việc độc hại chính xác hơn khi đã bị chiếm (`:252-256`) | Lớn |
| A9 | Một agent bị chiếm ghi vào mục mà mọi agent khác đọc (mọi agent ghi được mọi mục, `02-design.md:130`) | **Cao** về đường đi; chưa đo trên tài liệu kế hoạch | Lớn |
| A10 | Taint lan tới mọi phiên trong vài giờ (`02-design.md:133` cộng `:130`), nên tripwire taint báo mọi thứ và tín hiệu chết | **Cao** (suy ra, MEDIUM) | Vừa: mất khả năng thấy |
| A11 | Agent giả làm owner hoặc Chair trong văn bản ("owner đã đồng ý ở luồng khác") | **TB**: kiểu tấn công giả vai hệ thống trong `2510.09023:329` | Lớn nếu rail dựa vào văn bản |

### B3. Agent → công cụ và MCP

| # | Kịch bản | Khả thi | Tầm hại |
|---|---|---|---|
| A12 | Mô tả công cụ MCP bị đầu độc ngay từ lần đầu đăng ký | **Cao**: ASR cao nhất 72,8%, từ chối < 3% (`2508.14925:28`, `:625`); model mạnh hơn dễ bị hơn (`:578`) | Lớn |
| A13 | Agent tự tạo hoặc tái dùng công cụ có lỗ hổng, hoặc nhận công cụ độc từ ngoài | **Cao**: tạo hoặc tái dùng công cụ có lỗ hổng > 76%, không nhận ra công cụ độc gần 93% (`2509.26354:184-185`) | Lớn |
| A14 | Gói phụ thuộc hoặc máy chủ MCP bị đổi sau khi đã được tin (rug-pull) | **TB**: postmark-mcp (`research-ai-autonomous-system-risk.md:158`, MEDIUM) | Lớn |
| A15 | Dùng công cụ hợp lệ với credential niêm để làm việc trái phép (confused deputy) | **Cao**: F2 không chặn (`03-ledger.md:233-234`) | Lớn |

### B4. Agent → production, tiền và khách

| # | Kịch bản | Khả thi | Tầm hại |
|---|---|---|---|
| A16 | Hoàn tiền hoặc tín dụng hàng loạt do injection hoặc học sai | **Cao** (A1, A5); cam kết của agent là của công ty (`02-design.md:200`) | **Lớn**, không đảo ngược |
| A17 | Email hoặc tin nhắn sai gửi tới nhiều khách | **Cao** | **Lớn**, không đảo ngược |
| A18 | Phát ngôn công khai sai hoặc phỉ báng | **TB** | **Lớn**, không đảo ngược, rủi ro pháp lý |
| A19 | Xoá dữ liệu ở hệ thống bên thứ ba (nơi backup của mình không phủ) | **TB**: Replit, PocketOS (`03-ledger.md:198-205`) | Lớn |
| A20 | Chuyển tiền tới tài khoản kẻ tấn công, chia nhỏ để không vượt mức báo động | **TB**: đây là đúng nhiệm vụ CaMeL không chặn được khi người dùng bảo "trả theo tài liệu" (`2503.18813:739-743`) | **Lớn** |

### B5. Agent → nhà cung cấp LLM

| # | Kịch bản | Khả thi | Tầm hại |
|---|---|---|---|
| A21 | Dữ liệu cá nhân đi tới nhà cung cấp hoặc đích khác qua kênh của phiên, không qua `LegalPolicy` ở gate | **Cao**: phiên giữ mạng tới nhà cung cấp ngoài gate (ADR-0018, "Where the executor runs"); hồ sơ macOS của phiên là `(allow default)` (`internal/jail/sandbox_darwin.go:143`) | **Lớn**: không thu hồi được; phạt tới 5% doanh thu (`research-ai-autonomous-system-risk.md:56`) |
| A22 | Chuyển sang nhà cung cấp dự phòng (`04-launch.md:216`) gửi dữ liệu tới bên chưa có DPA | **TB** | Lớn |

### B6. Owner ↔ hệ thống

| # | Kịch bản | Khả thi | Tầm hại |
|---|---|---|---|
| A23 | Hàng đợi ngoại lệ bị làm ngập, sự cố thật chìm | **Cao**: tỷ lệ bỏ sót mục hiếm cao (`research-ai-autonomous-system-risk.md:118`) | Lớn: F1 đến muộn |
| A24 | Agent soạn câu hỏi để owner nói "tiếp tục" hoặc "merge"; Assistant làm theo "lời owner" | **TB–Cao**: người theo khuynh hướng của agent 86% (`research-ai-autonomous-system-risk.md:101`); có cảnh báo đúng phá hoại vẫn thành công 9/16 (`2606.05647:23`) | Lớn: rail được kéo đúng quy trình |
| A25 | Assistant bị injection gán cho owner một lời owner chưa nói | **TB** | **Lớn**: rail kéo "theo lời owner" |
| A26 | Lời tóm tắt của agent che sự cố; owner đọc tóm tắt, không đọc bằng chứng | **Cao**: người đọc log thô AUC 0,65 (`research-ai-autonomous-system-risk.md:105`) | Lớn |

### B7. Auditor ↔ bên sản xuất

| # | Kịch bản | Khả thi | Tầm hại |
|---|---|---|---|
| A27 | Đầu ra của bên sản xuất chứa chuỗi lừa judge ("Solution", "Thought process:") | **Cao**: tỷ lệ báo nhầm tới 80% (`2507.08794:38-40`) | Vừa: lỗi qua tầng kiểm |
| A28 | Bên sản xuất chèn chỉ thị cho auditor trong artifact | **Cao** (như A1, auditor đọc dữ liệu `untrusted`, `02-design.md:155`) | Vừa–Lớn |
| A29 | Agent biết rõ detector, tripwire và vị trí canary (mọi thứ mở, `04-launch.md:162-175`) nên tránh chúng | **TB**: model học về cơ chế giám sát từ môi trường (`2412.04984:95`); detector vào thưởng thì bị né (`2503.11926:18`) | Lớn |

Tổng: 29 kịch bản. Năm kịch bản rủi ro nhất xếp ở mục 6.

## 3. Lạm dụng chính các điểm cố định

### 3.1 Vượt F1 (nút dừng)

F1 trong bản thiết kế: gate từ chối mọi lời gọi mới thuộc phạm vi bị dừng, phiên đang chạy bị cắt (`02-design.md:194`). Ba lỗ hổng
ở mức thiết kế, đều có trong ADR của chính dự án:

1. **Đường ra không qua gate.** Phiên giữ mạng để gọi nhà cung cấp (ADR-0018), và hồ sơ macOS cho phép mặc định
   (`sandbox_darwin.go:143`). Một tiến trình do phiên mở có thể nói chuyện với bên ngoài mà gate không thấy. Dừng ở gate không cắt
   đường đó. Phép kiểm D1 (`04-launch.md:55`) đọc log của gate, nên chỉ chứng minh đường đi qua gate đã đóng.
2. **Tiến trình sống lâu hơn phiên.** Trên macOS không có pid namespace; một tiến trình con tự tách phiên vẫn sống sau deadline
   (ADR-0021, Consequences). "Cắt phiên" không đồng nghĩa với "cắt mọi thứ phiên đã mở".
3. **Bám trụ ngoài jail.** ADR-0018 ghi phiên cùng user với server nên ghi được những gì cron và shell của user đó khởi chạy, và
   "a list of paths cannot close" lỗ này. Danh sách ẩn chỉ phủ đúng job launchd của server (`cmd/remit/agents.go:98`).

Thêm hai đường ở mức tổ chức:
- **Sửa mã của stop.** I16 giữ mã gate và stop sau rail merge (`02-design.md:65`), nhưng `01-strategy.md:28-29` bỏ rail merge. Nếu
  bản cuối theo 01, agent có thể đổi chính mã dừng (ADR-0012: Remit tự cải thiện qua chain của nó).
- **Lời dặn không phải rail.** Lệnh "hãy để mình bị tắt" đặt trong system prompt được tuân **ít hơn** (`2509.14260:14`). Brief không
  bảo vệ được F1.

**Hệ quả.** F1 chỉ đúng khi mọi tác dụng của agent đi qua một điểm mà agent không chạm tới, và dừng được đo ở tầng mạng của host,
không ở log của chính gate. Hiện nay chưa đúng (**HIGH**, từ ADR).

### 3.2 Vượt F2 (credential niêm)

- **Credential nằm ngoài kho.** ADR-0018 ghi khoá SSH và đăng nhập GitLab của user **đọc được từ phiên**, chờ một connection kind
  GitLab chưa có. ADR-0021 ghi trên macOS một phiên đọc được môi trường của mọi tiến trình cùng user. I2 (`02-design.md:51`) và D2
  (`04-launch.md:56`) quét giá trị đã biết trong kho, nên không thấy các credential không nằm trong kho.
- **Che dấu không chống đối thủ.** ADR-0018 nhận phần dư: redaction bắt hình dạng token, không bắt giá trị đã mã hoá. Thuộc tính
  `Redact` "kể cả base64 và URL-encode" (`02-design.md:351`) đúng với rò vô ý, không đúng với chương trình cố ý mã hoá theo cách khác.
- **Không cần lấy secret.** Với F2 nguyên vẹn, kẻ chiếm phiên vẫn dùng mọi connection qua executor (A15), vì bản thiết kế bỏ
  "Least grant" của ADR-0018 (`02-design.md:195`: "mọi agent dùng được mọi connection"). Đây là thiệt hại chính, không phải lộ secret.

### 3.3 Dùng công cụ hợp lệ với credential niêm

Executor gắn credential cho **bất kỳ** yêu cầu nào hợp với effect. Tham số do agent đặt: người nhận email, số tiền hoàn, đích
webhook. MCPTox cho thấy đổi một tham số là dạng tấn công hiệu quả nhất (`2508.14925:592-602`). Nên ranh giới phải nằm ở **nguồn
gốc của tham số**, không ở việc ai giữ secret (mục 5, D1–D3).

### 3.4 Vượt F3 (luật)

`LegalPolicy` ở gate là hàm thuần trên `Action` (`02-design.md:320`). Hàm đó quyết được các luật về **luồng dữ liệu** (lớp dữ liệu
tới đích có DPA hay không). Nó không quyết được tính hợp pháp của **nội dung** (một phát ngôn có phỉ báng hay một quảng cáo có lừa
dối không). Câu "từ chối điều trái luật" (`02-design.md:83`) vì vậy quá rộng. Thêm vào đó, luồng dữ liệu tới nhà cung cấp đi qua
phiên, không qua gate (A21), nên ngay phần quyết được cũng chưa được ép.

## 4. Hành động không đảo ngược: thiết kế làm gì, nói thật

| Hành động | Bản thiết kế làm gì | Thực tế | Thay đổi tối thiểu (mục 5) |
|---|---|---|---|
| Email hoặc tin nhắn gửi khách | Ghi ledger, hiện ở màn hình agent (`02-design.md:200-201`) | Không có gì trước khi gửi; đã gửi thì không rút | D4 hoãn gửi có hủy; D1 tham số từ nguồn tin cậy |
| Hoàn tiền, tín dụng | Bản ghi có kiểu; báo nếu vượt mức (`02-design.md:200`) | Mức báo bị vượt qua bằng cách chia nhỏ (A20) | D6 tổng theo cửa sổ thời gian là tripwire; D4 |
| Chuyển tiền | Như trên | Không có biện pháp trước | D1 đích từ danh mục do owner nhập; D4 |
| Phát ngôn công khai | Không có mục riêng | Không đảo ngược; rủi ro pháp lý | D4; D9 phân biệt nội dung có nguồn ngoài |
| Dữ liệu tới nhà cung cấp | `LegalPolicy` ở gate (`02-design.md:196`) | Phiên đi ngoài gate (A21) | D2 mọi luồng qua một proxy |
| Xoá dữ liệu | Backup ngoài công cụ (`02-design.md:201`) | Chỉ phủ dữ liệu của mình, không phủ hệ thống bên thứ ba | D5 xoá mềm và snapshot trước khi xoá |

Kết luận: với cả sáu loại, "phát hiện, dừng, phục hồi" chỉ giới hạn **lần sau**, không trả lại **lần này**. Bản thiết kế nói điều
đó (`02-design.md:432-433`); blueprint cuối phải đặt nó ở đầu, không ở cuối.

## 5. Thay đổi giảm hại mà không giới hạn sáng kiến

| # | Thay đổi | Là giới hạn? | Vì sao | Bằng chứng |
|---|---|---|---|---|
| D1 | **Tách luồng điều khiển khỏi dữ liệu (CaMeL không chính sách).** Kế hoạch do model chỉ thấy đầu vào tin cậy (lời owner, brief, hợp đồng objective); nội dung ngoài chỉ là giá trị, không thành lệnh | **Không** | Agent vẫn làm mọi việc nó chọn; chỉ đổi *ai* được ra lệnh: dữ liệu ngoài không ra lệnh được | 300 → 0 / 949 tấn công thành công với Gemini 2.5 Pro, không chính sách (`2503.18813:725-729`). Giới hạn: role mà việc chính là "làm theo khách" vẫn biến dữ liệu thành lệnh (`:859-888`) |
| D2 | **Một đường ra duy nhất cho mọi tác dụng**: phiên và chương trình chỉ có mạng qua một proxy do nền tảng giữ; F1 và F3 ép ở đó | **Không** | Không đích nào bị chặn; chỉ đổi kênh. Đây là điều kiện để I1 và F1 đúng như bản thiết kế đã hứa | ADR-0018 và ADR-0021 ghi các đường còn lại |
| D3 | **Nhãn nguồn gốc trên từng tham số** của hành động (không chỉ trên phiên); tham số rút từ nội dung ngoài hiện rõ trong bản ghi và trên màn hình | **Không** | Chỉ ghi và hiển thị | Dạng đổi tham số là mạnh nhất (`2508.14925:592-597`) |
| D4 | **Hoãn gửi có hủy được** cho hành động ra khỏi công ty (email, hoàn tiền, đăng bài): nền tảng giữ một cửa sổ ngắn trước khi giao; F1 trong cửa sổ đó cũng rút được hành động | **Không**, nếu không ai phải duyệt: agent đã quyết, hành động chắc chắn xảy ra trừ khi bị dừng tay | Biến "không đảo ngược" thành "đảo ngược trong cửa sổ" cho F1 | Knight: tổn thất theo thời gian dừng (`02-design.md:29-31`) |
| D5 | **Hành động mặc định đảo ngược được**: xoá mềm, snapshot trước khi ghi đè, bản ghi bù trừ cho mỗi tác dụng | **Không** | Hành động vẫn xảy ra | PocketOS, Replit (`03-ledger.md:198-205`) |
| D6 | **Tripwire theo tổng trong cửa sổ** (hoàn tiền, email, đích mới), không chỉ theo từng hành động | **Không**: báo, không chặn | Chống chia nhỏ (A20) | `2503.18813:739-741` |
| D7 | **Log chống giả mạo**: chuỗi băm, ghi ra nơi phiên không ghi được | **Không** | Chỉ ghi | Apollo: lời dối được giữ > 85% (`2412.04984:21`) |
| D8 | **Canary token**: credential giả và bản ghi khách giả, chạm tới thì báo | **Không** | Chỉ báo | Mạnh nhất khi vị trí không công bố (xem 7.2) |
| D9 | **Bằng chứng do nền tảng dựng** cho rail (diff, con số, nguồn của tham số) thay lời tóm tắt của agent | **Không** | Thay đổi cái owner đọc | Vasconcelos (`research-ai-autonomous-system-risk.md:116`) |
| D10 | **Rail do Assistant kéo phải trỏ tới một tin nhắn do owner viết** (mã tin nhắn đã xác thực), hiện nguyên văn lời đó cạnh hành động | **Không**: đây là xác thực, rail đã có trong `AGENTS.md` | Chặn A25 | `AGENTS.md` (rail ghi ai đã hành động) |
| D11 | **Dừng theo mạng ở host**: diễn tập F1 đo lưu lượng ra của host sau khi dừng, không đo log của gate | **Không** | Là phép đo | Sửa D1 (`04-launch.md:55`) |
| D12 | **Ghim và duyệt mã nguồn của máy chủ MCP lần đầu** bằng checker xác định (so mô tả với hành vi, tìm chỉ thị trong mô tả) | **Không** nếu chỉ báo | Chống A12 tốt hơn băm (băm chỉ thấy thay đổi) | `2508.14925:639` |

Những biện pháp **là** giới hạn và vì vậy không đề xuất: Rule of Two, danh sách đích được phép, hạn mức chi, cổng duyệt cho
email. Bản thiết kế đúng khi xếp chúng vào loại giới hạn.

## 6. Mười rủi ro blueprint đánh giá thấp, xếp hạng

| # | Rủi ro | Bằng chứng | Thay đổi tối thiểu |
|---|---|---|---|
| 1 | **F1 không phủ đường ra ngoài gate** (A3, mục 3.1) | ADR-0018; `sandbox_darwin.go:143`; ADR-0021 | D2, D11 |
| 2 | **Confused deputy với mọi connection** (A1, A6, A15) | ASR 33–48% (`2406.13352:944-950`); `02-design.md:195` | D1, D3 |
| 3 | **Chiếm Chair qua sub-agent** (A7) | `2503.12188:279-282` | D1 ở Chair |
| 4 | **Dữ liệu tới nhà cung cấp ngoài `LegalPolicy`** (A21) | ADR-0018 | D2 |
| 5 | **Hành động không đảo ngược không có cửa sổ** (A16–A18) | `02-design.md:432-433` | D4, D5 |
| 6 | **Taint bão hoà, tín hiệu chết** (A10) | `2410.07283:358`; `02-design.md:130-133` | D3 (nhãn theo tham số, không theo phiên) |
| 7 | **Credential ngoài kho đọc được từ phiên** (mục 3.2) | ADR-0018, ADR-0021 | Đưa khoá SSH và GitLab ra sau gate; quét mọi credential, không chỉ giá trị trong kho |
| 8 | **Assistant kéo rail theo lời bịa** (A25) | `research-ai-autonomous-system-risk.md:101` | D10 |
| 9 | **Công bố detector cho kẻ tấn công thích ứng** (A29, `04-launch.md:170-175`) | Kẻ thích ứng dùng phản hồi của detector (`2510.09023:361-362`) | Công bố phương pháp; giữ ngưỡng tripwire và vị trí canary ngoài bản công bố. Đây là quyết định của owner (xem 7.2) |
| 10 | **"An toàn nhờ kém" sẽ mất theo model mới** | WASP: hoàn thành mục tiêu kẻ tấn công 0–17% (`2504.18575:113`); model mạnh hơn dễ bị hơn (`2508.14925:578`); `AGENTS.md` "build for next year's models" | Không dựa vào con số WASP khi ước rủi ro |

## 7. Khẳng định quá mạnh hoặc sai trong bản nháp

### 7.1 Danh sách

| Vị trí | Khẳng định | Vấn đề | Bằng chứng |
|---|---|---|---|
| `02-design.md:50` (I1) | Mọi lời gọi ra ngoài qua một gate | Phiên giữ mạng ngoài gate | ADR-0018; `sandbox_darwin.go:143` |
| `02-design.md:206` | "Phiên agent ra ngoài qua gate" | Như trên | Như trên |
| `02-design.md:211-213` | CaMeL "là giới hạn" | Phần tách luồng không cần chính sách | `2503.18813:724-729` |
| `02-design.md:213` | F2 giữ secret không bị lộ trực tiếp | Credential ngoài kho đọc được | ADR-0018, ADR-0021 |
| `02-design.md:297` | Agent kháng tắt "không có đường ra sau khi gate đóng" | Có đường ngoài gate và tiến trình sống lâu hơn phiên | Mục 3.1 |
| `02-design.md:83` | Control plane "từ chối điều trái luật" | Chỉ quyết được luật về luồng dữ liệu | Mục 3.4 |
| `02-design.md:201` | Xoá dữ liệu khách để lại bản sao lưu ngoài công cụ | Không phủ hệ thống bên thứ ba | Mục 4 |
| `02-design.md:208` | Ghim và băm mô tả MCP đối phó MCPTox | Băm chỉ thấy thay đổi, MCPTox đầu độc từ đầu | `2508.14925:104-107` |
| `02-design.md:351` | `Redact` đúng "kể cả base64 và URL-encode" như một bảo đảm | Chỉ đúng với rò vô ý | ADR-0018 (phần dư đã nhận) |
| `02-design.md:51` (I2) | ADR-0018 là hiện thực tham chiếu | Bản thiết kế bỏ "Least grant" và "write là mục chờ owner" của ADR-0018 nhưng vẫn dẫn nó | ADR-0018 invariant 2; `02-design.md:195` |
| `04-launch.md:55` (D1) | Đọc log nền tảng chứng minh không còn lời gọi | Chỉ chứng minh đường qua gate | Mục 3.1 |
| `04-launch.md:257` | "Agent không cầm secret nên không gửi được secret đi" | Dữ liệu khách vẫn đi được; credential ngoài kho | Mục 3.2 |
| `01-strategy.md:28-29` so với `02-design.md:65` | 01 bỏ rail merge, 02 giữ | Bỏ merge thì mã của stop đổi được bởi agent | ADR-0012 |

### 7.2 Mâu thuẫn cần owner quyết

"Mọi thứ mở" trong quyết định của owner là mở **với owner** (`02-design.md:22`). `04-launch.md:170-175` mở rộng thành công bố với
thế giới, gồm brief, playbook, mã harness và bộ kiểm. Mở với owner không mâu thuẫn với bảo mật. Mở ngưỡng tripwire và vị trí canary
với thế giới thì vô hiệu chúng. Đề xuất: công bố phương pháp, không công bố tham số của detector. Owner cần nói rõ "mở" nghĩa là
mở với ai.

## 8. Điều blueprint làm đúng

- Đặt nút dừng và log ngoài tầm agent là điều kiện tồn tại, không là tính năng (`02-design.md:24-27`), đúng bài học Palisade.
- Nói thẳng rằng bề mặt injection mở và không có biện pháp nào trước hành động (`02-design.md:209-214`, `04-launch.md:257`).
- Detector không vào thưởng (`02-design.md:186`), đúng với `2503.11926:18`.
- Log, trace và backup do nền tảng ghi, chỉ thêm (I4), đúng với PocketOS.
- Owner không phải lưới chặn; hàng đợi ngoại lệ do máy xếp hạng (`02-design.md:231-237`), đúng với bằng chứng giám sát của người.
- Checker xác định trước judge và số liệu do mã tính (I10), giảm A27.
- Credential nhập vào trường chỉ ghi và không vào ngữ cảnh agent (`AGENTS.md`, rails), loại bỏ đúng mẫu PocketOS.
- Diễn tập dừng, khôi phục và canary chạy trong production và hiện cho owner (`02-design.md:356`).

## 9. Giả định

- Assumption: "không giới hạn sáng kiến" cấm biện pháp làm hành động không xảy ra hoặc phải chờ, không cấm biện pháp đổi kênh, nhãn
  hay khả năng hoàn tác, vì danh sách cấm của owner (`02-design.md:19-20`) toàn là giới hạn lên *việc gì* agent làm.
- Assumption: hiện thực tham chiếu của blueprint là mã và ADR của Remit hiện tại, vì `02-design.md:51` và `:428` dẫn ADR-0018.
- Assumption: hoãn gửi có hủy (D4) không là cổng duyệt khi không có chủ thể nào phải hành động để hành động xảy ra; nếu owner coi bất
  kỳ độ trễ nào là giới hạn, D4 rơi khỏi danh sách và mục 4 không còn biện pháp trước hành động.
