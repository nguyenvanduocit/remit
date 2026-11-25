# Chiến lược: công ty vận hành bằng agent (v2)

> Bản nháp của hội đồng, phần 1/4 (chiến lược), bản 2, sau sáu phản biện. Ngày 2026-11-01. Bản 1: [`01-strategy-v1.md`](01-strategy-v1.md);
> nhật ký trả lời từng phản biện: [`01-response.md`](01-response.md). Viết tắt nguồn:
> **[E]** [`research-…-evidence.md`](../research-ai-autonomous-system-evidence.md), **[D]** [`research-…-design.md`](../research-ai-autonomous-system-design.md),
> **[R]** [`research-…-risk.md`](../research-ai-autonomous-system-risk.md), **[P]** [`research-…-pilot.md`](../research-ai-autonomous-system-pilot.md);
> **[C01]…[C06]**: `challenge-0N-*.md` trong thư mục này (phản biện của hội đồng; nhãn là của người phản biện). `§` là số mục.
> Nhãn: **HIGH** (đã đối chiếu nguồn gốc), **MEDIUM** (suy ra, nguồn một phía, vendor, hoặc chỉ qua phản biện), **LOW** (phỏng đoán).
> Đề xuất của bài này ghi **Đề xuất của hội đồng (giả thuyết)**. Sổ claim ở mục 8.

## 0. Khung quyết định

**Quyết định của owner (cố định):** toàn bộ hệ thống bật một lần trong một ranh giới và được sửa trong lúc chạy; không thí nghiệm;
không bí mật; không giới hạn sáng kiến của agent. Ba điểm cố định: **(1)** owner dừng tay được mọi lúc, nút dừng nằm ngoài môi trường
agent sửa được; **(2)** credential không bao giờ nằm trong ngữ cảnh agent; **(3)** tuân thủ pháp luật.

**Phán quyết của chủ tọa** (mặc định, chờ owner xác nhận; bài này viết theo chúng):

| # | Phán quyết |
|---|---|
| R1 | Các rail xác định của `AGENTS.md` giữ nguyên: xác thực, dừng và tạm dừng, merge, credential trong trường chỉ ghi. Chúng là quyết định trước đó của chính owner, và bỏ merge thì agent sửa được chính mã của nút dừng (Remit tự cải thiện qua chain của nó, ADR-0012). "Không cổng duyệt" nghĩa là không cổng nào trên sáng kiến của agent bên trong nền tảng, ngoài các rail này. |
| R2 | "Mở" là mở với owner và người của công ty. Phương pháp công bố ra ngoài sau khi một người có tên kiểm pháp lý. Ngưỡng phát hiện, vị trí canary, tập neo giữ lại và nhãn lỗi gieo **niêm** với agent bị đo và với công chúng: tập đánh giá niêm là ngoại lệ thứ hai, cạnh credential. |
| R3 | Lặp cùng một tác vụ nhiều lần để đo độ nhất quán là **quan sát**, được phép. Chia nhánh ngẫu nhiên, holdout và A/B trên khách là thí nghiệm, không được phép. |
| R4 | Luật thắng. Thứ luật buộc quay lại (kiểm soát và giám sát của người nơi luật đòi, giới hạn mục đích và vai trò với dữ liệu cá nhân, báo cho người ngoài rằng họ nói với AI, hồ sơ do một người có tên ký, thủ tục và lịch luật lao động khi thay người, kiểm soát thay đổi SOX cho bên niêm yết, báo cáo sự cố nghiêm trọng) là **hệ quả của điểm cố định 3**, không phải giới hạn do owner chọn. |
| R5 | Hiệu ứng chia thành đảo ngược được và không đảo ngược. Với loại không đảo ngược, điều kiện "loss is bounded and recovery is cheap" của `AGENTS.md` **không thoả**, và bài này nói thẳng như vậy. Hoãn-có-huỷ là lựa chọn owner có thể bật, mặc định tắt. |
| R6 | Định vị: một tổ hợp có kỷ luật các ý đã công bố, cộng một lựa chọn thiết kế lệch chuẩn và chưa được chứng minh. Không dùng "mới", "đầu tiên", "duy nhất", "mọi công ty", "bước tiến của nhân loại". |
| R7 | Remit là hiện thực tham chiếu, không phải bằng chứng; các lỗ đã biết của nó được liệt kê (mục 7.2). |
| R8 | Mọi con số được sửa theo [C01]. |
| R9 | ROI hôm nay không tính được: bài đưa công thức, độ nhạy và các đầu vào chưa có nguồn. Chi phí do công ty áp dụng đọc, bên ngoài hệ agent (ADR-0010). |
| R10 | Ranh giới là đơn vị do bên áp dụng chọn. Đơn vị bên ngoài là nhóm so sánh tự nhiên cho phân tích quan sát (difference-in-differences): phân tích biến thiên đã có, không phải thí nghiệm. |

Niềm tin dành cho hướng đi (agent sẽ làm phần lớn hơn công việc của công ty; năng lực sinh rẻ dần). Nghi ngờ dành cho dữ kiện.

## 1. Luận điểm và bài toán của doanh nghiệp lớn

**Luận điểm đã có người nêu.** "Kiểm chứng là điểm nghẽn, năng lực sinh thì không" đã được công bố: verifier's law (Wei, 2025),
generation-verification loop (Karpathy, 06/2025) [C06 §0, MEDIUM], và "contract-first decomposition" của DeepMind, theo đó chỉ giao
việc khi kết quả "has precise verification", nếu không thì phân rã tiếp (Tomašev, Franklin, Osindero, 2602.11865, HIGH, đã đọc PDF).
Bài này **áp ý đó vào cấp công ty**: mua năng lực sinh, tự xây năng lực kiểm chứng, và đo chính bộ kiểm chứng.

**Dạng có thể sai của luận điểm (Đề xuất của hội đồng, giả thuyết).** Ở lớp việc có checker xác định, chi phí trọn gói mỗi đơn vị đứng
vững (mục 3) trong tháng 9–12 sau ngày 0 thấp hơn so với cùng lớp việc ở các đơn vị bên ngoài ranh giới cùng kỳ (R10). Nếu không,
luận điểm **không được ủng hộ** ở lớp việc đó.

**Bài toán.** Doanh nghiệp đã mua AI; điểm nghẽn dời sang người duyệt:

- Công ty phần mềm 802 lập trình viên, 196.212 PR: thông lượng tăng khoảng 12% khi từng người bắt đầu dùng AI, tiếp tục tăng, đạt 2×
  chỉ sau khoảng chín tháng dùng; **chi phí dồn về code review, tải mỗi reviewer gấp đôi** [2607.01904, HIGH]. Bài này cho thấy AI
  **có** tăng thông lượng ở doanh nghiệp; chỉ vế chi phí review ủng hộ luận điểm.
- Remit trên chính nó: 52 merge request chờ owner khi đo [ADR-0002:40, HIGH; một hệ thống, một người duyệt].
- Faros (10.000+ dev): thời gian review +91%, không thấy tương quan giữa mức dùng AI và cải thiện ở cấp công ty [C05 §4, MEDIUM, vendor].
- Lãnh đạo ở 7 nước báo năng suất +0,2%/năm do AI [P §3; NBER w34836, HIGH, tự báo cáo].

Đọc theo đầu tư: tiền đã chi cho sức sinh; lợi tức bị chặn ở sự chú ý của người duyệt. **MEDIUM** (suy ra từ các nguồn trên).

## 2. Vì sao bây giờ, và bài này ghép lại những gì

### 2.1 Vì sao bây giờ

1. **Năng lực tăng nhanh hơn độ tin cậy.** Độ dài tác vụ agent làm được ở mức 50% tăng gấp đôi khoảng mỗi 7 tháng [2503.14499, HIGH];
   ước tính gần đây khoảng 131 ngày [D §8, MEDIUM]. Trên 15 model qua 24 tháng, độ tin cậy tăng 0,03/năm (GAIA) và 0,09/năm (τ-bench),
   độ chính xác tăng 0,23/năm và 0,22/năm; mỗi đơn vị độ chính xác chỉ kéo độ tin cậy lên 0,18 và 0,33 [2602.16666, Hình 1 và tr. 38,
   HIGH]. Hệ quả (**MEDIUM**): chờ model tốt hơn không tự giải bài toán tin cậy; phần đó là việc của hệ thống bao quanh model.
2. **Giá token ở năng lực cố định giảm 9–900 lần/năm; giá ở biên frontier thì không** [D §8, MEDIUM]. Năng lực sinh là thứ mua được.
3. **Luật đã có hiệu lực, không có thời gian chuyển tiếp cho hệ thống mới.** Luật AI Việt Nam hiệu lực 01/03/2026; thời gian chuyển tiếp
   (tới 01/09/2027 cho tài chính) chỉ cho hệ thống đã hoạt động trước ngày đó, nên hệ thống bật ở ngày 0 tuân thủ ngay khi đưa vào dùng
   [C04 K9, HIGH theo văn bản cơ sở dữ liệu]. Hệ thống sinh nội dung bật sau 02/08/2026 phải đánh dấu máy đọc ngay (AI Act Điều 50(2))
   [C04 K3, HIGH theo OJ].

### 2.2 Prior art và cái bài này lấy

| Prior art | Đã làm | Bài này lấy gì, khác ở đâu |
|---|---|---|
| Anthropic multi-agent research system | Lead agent và subagent; +90,2% so với agent đơn trên eval nội bộ; khoảng 15 lần token của chat [E §3, MEDIUM] | Lấy điều phối; thêm việc chain giữ kết quả đến khi hiệu ứng được kiểm chứng (ADR-0013) |
| Magentic-One; MetaGPT, ChatDev; OKR-Agent (2311.16542) | Sổ task và progress do orchestrator giữ [2411.04468, HIGH]; role có artifact, số lần người sửa 10 → 2,5 [2308.00352, HIGH]; phân rã objective thành OKR theo tầng [C06 §1.1, MEDIUM] | Lấy sổ chung và phân rã OKR; khác: duyệt dựa trên kiểm chứng có bằng chứng [2609.14767] |
| Tomašev et al., 2602.11865 | Contract-first decomposition, ledger bất biến, panel phân xử | Gần nhất về lý thuyết. Khác: DeepMind phân rã đến khi kiểm được; bài này cho phép chạy việc chưa kiểm được và chỉ ghi lại (lựa chọn chưa có số đo) |
| Paperclip (2026, "zero-human companies") | Org chart agent, goal → task, cấu hình có phiên bản, log, secret injection, trần ngân sách, board approval [C06 §1.4, HIGH cho README] | Prior art gần nhất cho cả hệ. Khác: bài này không có trần chi phí hay cổng duyệt trên sáng kiến; có checker xác định, đo độc lập judge |
| Nền tảng doanh nghiệp: OpenAI Frontier, Microsoft Agent 365, ServiceNow AI Control Tower, Salesforce Agentforce | Control plane, định danh agent, quản trị, hợp đồng sẵn có [C05 §4, C06 §1.3, MEDIUM] | Họ thắng ở mua sắm, định danh, lưu trú dữ liệu, chứng chỉ. Bài này đóng góp phép đo và kiểm chứng sau ship |
| Niêm credential: Auth0 Token Vault, Arcade, AWS Bedrock AgentCore Identity | Agent gọi API mà không giữ token [C05 §4, C06 §1.3, MEDIUM] | Thực hành có sẵn; bài này dùng nó làm điểm cố định 2, không phải đóng góp |
| Devin, Cursor cloud agents | Nhận việc, mở PR, chạy theo lịch hoặc sự kiện; Devin tự báo 67% PR được merge năm 2025 (34% năm trước) và cần người duyệt "when outcomes aren't straightforwardly verifiable" [cognition.com, cursor.com] | Việc được theo dõi tiếp sau merge (ADR-0013); owner vẫn kéo merge (R1). Giả thuyết: agent xây được kiểm chứng cho một phần việc Devin nêu là khó kiểm; chưa có số đo |
| Khung quản trị: IMDA Singapore (01/2026), OpenAI (Shavit 2023), Google (2025), NIST NCCoE | Đều khuyên giới hạn quyền, phạm vi hành động, điểm duyệt của người [C06 §1.2, HIGH cho IMDA] | Bài này **đi ngược** ở điểm đó (mục 2.3) |
| Klarna | AI xử lý 2/3 chat, +$40 triệu ước tính 2024; 05/2025 tuyển lại người [D §8] | Chất lượng cho khách đọc từ tín hiệu ngoài, không từ báo cáo của agent |
| Project Vend, CEO-Bench, Andon, HurumoAI, Polsia | Không ca hạng A hoặc B nào có lợi nhuận đã xác minh [P §3]; mọi model trong CEO-Bench thua baseline theo quy tắc [2606.18543, HIGH] | Không thêm được gì đã chứng minh |

### 2.3 Đóng góp, nói chính xác

Không thành phần nào là phát minh. Theo [C06 §2]: ba thành phần đã công bố (checker xác định cho số khách, hàng đợi ngoại lệ, triển khai
một lần), bốn là tổ hợp (phân rã objective có kiểm chứng, ngữ cảnh chung có kiểu, đo độc lập judge, scorecard công khai), và **một lựa
chọn lệch mọi khung đã công bố**: gate chỉ từ chối vì dừng và luật, ngoài các rail cố định ở R1 (những việc agent không bao giờ làm:
merge, đăng nhập, cầm credential). Mọi khung ở bảng trên khuyên điều ngược lại; vì thế độ lệch này là rủi ro, không phải bằng chứng.

**Câu đóng góp (theo [C06 §5], MEDIUM):** blueprint đóng góp một chuỗi dữ liệu thực địa mở, n = 1 tổ chức, về ba đại lượng tài liệu hiện chỉ
đo trên benchmark: (a) tỷ lệ bắt lỗi của checker và auditor, đo bằng lỗi gieo từ lịch sử; (b) độ độc lập thực của panel judge trên dữ liệu
kinh doanh thật (ρ và n_eff trên tập neo); (c) tỷ lệ lỗi thoát trong 30 ngày theo lớp việc, ở cấu hình tự chủ mà khảo sát 86 hệ thống
(68% chạy tối đa 10 bước trước khi người can thiệp) không phủ [2512.04123]. Đóng góp này **không** gồm bằng chứng nhân quả về lợi nhuận.

**Mức cao nhất bằng chứng cho phép nói hôm nay:** "Một phương pháp vận hành mở, có thể bác bỏ được, cho công ty giao phần lớn việc cho
agent, cùng cam kết công bố dữ liệu vận hành thật, kể cả thất bại." [C06 §4]

## 3. Mô hình giá trị

### 3.1 Nguồn lợi tức

| Nguồn | Cơ chế | Bằng chứng |
|---|---|---|
| R1. Thông lượng trên việc kiểm chứng được | Agent làm việc có test, đối soát, quy tắc | Thông lượng PR tăng tới 2× sau khoảng chín tháng dùng [2607.01904, HIGH]; agent đơn tự hoàn thành khoảng 30% tác vụ công ty mô phỏng, cao nhất ở kỹ thuật phần mềm [2412.14161, HIGH] |
| R2. Chu kỳ ý tưởng → phản hồi ngắn hơn | Việc chạy liên tục; tín hiệu quay về chain làm ra nó | Chưa có số đo. **LOW** |
| R3. Lỗi âm thầm bị bắt sớm hơn | Auditor đối soát nguồn thô | Chỉ rút ngắn thời gian tới lúc sửa với hiệu ứng đảo ngược được; không trả lại hiệu ứng không đảo ngược (mục 4.3). **LOW** |
| R4. Tài sản kiểm chứng tái dùng | Mỗi lỗi đã gặp thành một ca kiểm | Chưa có số đo. **LOW** |

Không có lợi tức nào từ "cắt 80% chi phí review/QA": không nghiên cứu nào ủng hộ, và dữ liệu đi ngược lại [D §8, P §6; HIGH rằng không có].

### 3.2 Công thức, khi mọi thứ chạy cùng lúc

Không có trần chi phí và không có chi tiêu theo bậc. Theo kỳ `t`, lớp việc `k` (dựng lại theo [C05 §1.1]):

```
Lợi ích(t) = Σ_k N_k · s_k · a_k · H_k · w · c_k   (+ ΔDoanh thu: chỉ ước được bằng so sánh với đơn vị ngoài ranh giới)
Chi phí(t) = S(t) + Σ_k N_k · r_k · w + D(t) + E[L](t) + G(t) + F0/H + F(t)
```

| Biến | Nghĩa | Tình trạng nguồn |
|---|---|---|
| `S(t)` | Tổng chi cho agent, kể cả chain bỏ dở | Một mẫu dogfooding (dưới) |
| `N_k` | Đơn vị agent tự nhận | Quan sát; không bị chặn |
| `s_k` | Tỷ lệ đơn vị **có cầu thật** (một người lẽ ra sẽ làm việc đó) | **Không có nguồn**; 83% chi đo được ở Remit là việc nó tự giao [ADR-0002:38] |
| `a_k` | Tỷ lệ đứng vững 30 ngày (không mở lại, không hoàn tác, không khiếu nại) | **Không có nguồn** cho hệ này |
| `H_k`, `w` | Giờ người tương đương; đơn giá giờ có gánh chi phí | `H_k` không có nguồn; `w` khoảng $120/giờ kỹ sư [D §8, Ord, MEDIUM] |
| `c_k` | Tỷ lệ giờ giải phóng **thành tiền** | **Không có nguồn**; ServiceNow tái đầu tư $100M tiết kiệm headcount thay vì vào biên lợi nhuận [C05 §1.1, MEDIUM] |
| `r_k` | Giờ người duyệt mỗi đơn vị | Chỉ có hướng: tải reviewer gấp đôi [2607.01904] |
| `D(t)` | Nợ kỹ thuật quy ra tiền | Chỉ có hướng: +30,26% cảnh báo tĩnh, +41,64% độ phức tạp, kéo dài [2511.04427, HIGH] |
| `E[L]` | Tổn thất kỳ vọng sau phần bảo hiểm trả | Không có tần suất; đuôi dày (Knight Capital lỗ hơn 460 triệu USD trong khoảng 45 phút [E §5]); bảo hiểm đang loại trừ AI (mục 7.4) |
| `G(t)` | Chi phí tuân thủ và quản trị (hồ sơ, DPIA, tham vấn người lao động, kiểm toán) | Không có số; bắt buộc theo R4 |
| `F0`, `H`, `F(t)` | Dựng ngày 0, khấu hao, sửa trong lúc chạy | Giả định của tài chính |

**Một mẫu dogfooding.** ADR-0002 (nay bị ADR-0010 thay): $1.126,07 trong 3,5 ngày, khoảng 114 chain, 8 MR merge, khoảng $141 trọn gói
mỗi MR; chi phí riêng của 8 chain đã merge là 8 × $8,25 = $66, tức **5,9% tổng chi** [ADR-0002:25–26, HIGH cho số]. Một trong hai ngày đo có
lẽ chỉ là nửa ngày; 83% chi là việc Scout tự giao, và Scout nay đã bỏ (ADR-0023). Cơ sở chi phí có thể là giá tương đương API do Claude Code
báo chứ không phải tiền mặt trả theo thuê bao (ADR-0002:58; [C05 §1.2], **LOW**). n = 8: không suy rộng được.

**Độ nhạy (minh hoạ, không phải ước tính).** Hoà vốn cho một MR khi `s·a·H·w ≥ $141 + r·w`. Các giá trị `r`, `s`, `a` dưới đây là giả định
của [C05 §1.2], không có nguồn; hội đồng tính lại từng dòng:

| `w` | `r` | `s` | `a` | Giờ người tương đương cần có để hoà vốn |
|---|---|---|---|---|
| $120 | 0 | 1 | 1 | 1,18 |
| $120 | 0,5 h | 1 | 1 | 1,68 |
| $120 | 0,5 h | 0,5 | 0,9 | 3,72 |
| $120 | 0,5 h | 0,17 | 0,9 | 10,9 |
| $120, giá token giảm 10 lần | 0,5 h | 1 | 1 | 0,62 |

**Kết luận (HIGH cho cấu trúc, LOW cho mọi con số):** ROI hôm nay không tính được cho lớp việc nào. Dấu của nó do năng lực review, `s` và
`c` quyết định, không do giá token; cả ba nằm trong tay doanh nghiệp. Khi số việc chờ duyệt vượt năng lực review, chi phí mỗi đơn vị chấp
nhận tăng theo chi tiêu mà không có trần [C05 §1.3]: đó là lý do câu hỏi về trần hàng đợi (mục 7.2, chuyển lên owner) là câu hỏi kinh tế.

**Ai đọc chi phí.** ADR-0010 tự ghi điều kiện xét lại: "runs for someone other than its owner" (ADR-0010:43). Một doanh nghiệp áp dụng là
đúng trường hợp đó. Theo R9: bộ phận tài chính của bên áp dụng đọc chi phí từ hoá đơn nhà cung cấp, theo lớp việc, từ ngày 0, bên ngoài hệ
agent; con số mở với người của công ty (R2) và không thành mục tiêu hay phần thưởng của agent, vì chỉ số đưa vào phần thưởng bị tối ưu lệch
hoặc bị né [E §4; 2210.10760, 2503.11926, HIGH].

**Cửa sổ đọc.** Hai nguồn kéo hai hướng: lợi ích tốc độ tan sau khoảng hai tháng ở dự án mã nguồn mở [2511.04427] còn thông lượng doanh nghiệp
đạt 2× chỉ sau khoảng chín tháng [2607.01904]. Vậy nợ và lợi ích ảo đọc từ tháng thứ ba; kết luận về lợi tức không trước tháng 9–12.
**MEDIUM**.

### 3.3 Chi phí cơ hội của việc chờ

Năng lực sinh: chờ gần như không mất gì, nó mua được và rẻ dần [D §8]. Năng lực kiểm chứng: gắn với dữ liệu, khách và lỗi của chính công
ty, nên chỉ tích luỹ bằng vận hành. Đại lượng quyết định là **thời gian T để một công ty dựng được bộ kiểm chứng đạt một mức bắt lỗi D trên
lỗi gieo**. T chưa được đo ở đâu; nếu T ngắn, chờ gần như không tốn gì (**LOW**). Bật một lần tăng lượng dữ liệu lỗi mỗi tuần nhưng làm khó
quy nguyên nhân hơn; lợi ròng cho việc học chưa biết dấu (**LOW**).

## 4. Áp dụng một lần trong một ranh giới

### 4.1 Ranh giới

Ranh giới là đơn vị bên áp dụng chọn: một tenant hay nhóm khách, một đơn vị kinh doanh, một tập chức năng (R10). Bên trong, mọi thứ bật
cùng lúc. Ba hệ quả của điểm cố định, không phải giới hạn chọn thêm:

- Ranh giới chỉ gồm hệ thống mà mọi tác dụng của agent đi qua gate, vì nút dừng (điểm 1) và niêm credential (điểm 2) chỉ đúng ở đó [C02 §4 #5,
  C03 §3.1].
- Luật áp theo nơi khách và pháp nhân ở (R4, mục 4.4).
- Đơn vị bên ngoài là nhóm so sánh tự nhiên (R10). Chiến lược nhận dạng (đơn vị so sánh, chỉ số, ngưỡng đọc) được công bố trước ngày 0;
  phương pháp gần nhân quả trên dữ liệu quan sát gồm difference-in-differences, synthetic control, CausalImpact [C06 §4, MEDIUM].

"Nơi bằng chứng mạnh nhất" là **quy tắc chọn ranh giới**: chọn đơn vị mà việc bên trong phần lớn kiểm chứng được bằng máy. Lợi tức đến sớm ở
việc có test và kỹ thuật phần mềm [2412.14161]; +80,8% của kiến trúc tập trung đo trên suy luận tài chính phân rã được, còn SWE-bench Verified
giảm nhẹ ở mọi kiến trúc nhiều agent [2512.08296, HIGH]: chọn ít agent cho việc code, nhiều agent cho việc phân rã được. Thất bại đến sớm ở
hành chính, tài chính, khoa học dữ liệu [2412.14161], lập kế hoạch tuần tự (tới −70%) [2512.08296], và hỗ trợ khách, nơi 80,1% lần đạt kết quả
vi phạm quy trình (55,5% của mọi 840 lần chạy) [2610.25954, HIGH].

### 4.2 Bật cùng lúc ở ngày 0

| Thành phần | Việc | Bằng chứng định hình cách làm |
|---|---|---|
| Role agent và Chair | Làm việc; phân rã một objective của owner thành OKR | Phần "make it work" |
| Reviewer agent | Đối chiếu với test, dữ liệu, hợp đồng của objective cha; không kiểm chứng được thì nói rõ, không lặp góp ý | Duyệt chỉ góp ý làm xấu kết quả, duyệt kiểm chứng được thì có ích [2609.14767; HIGH cho số đo, MEDIUM cho khái quát: 43 cặp, một loại việc] |
| Auditor agent | Đối soát nguồn thô, khác pipeline với bên sản xuất; phép kiểm do agent khác tác giả của nó duyệt (ADR-0013) | Judge sai tương quan cả khi khác nhà cung cấp [2605.29800, 2609.22512, HIGH] |
| Tập đánh giá niêm (R2) | Tập neo, nhãn lỗi gieo, một lát bộ hồi quy; owner thấy hết, agent bị đo không thấy | Holdout dùng lặp lại bị overfit [1411.2664, 1502.04585]; detector vào thưởng thì bị né [2503.11926] |
| Đo nhất quán (R3) | Lặp K lần cùng tác vụ ghi lại; báo cáo pass^k và lần chạy tệ nhất | Một lần chạy nói quá độ tin cậy: pass^8 dưới 25% ở bán lẻ [2406.12045] |
| Tín hiệu sau ship | Mỗi thay đổi mang cách đọc hiệu ứng; chain giữ kết quả đến khi đọc xong (ADR-0013) | Chỉ số ngắn hạn không dự đoán được dài hạn [D §7, HIGH] |
| Bề mặt quan sát | Mọi hành động, lý do, lỗi hiện nơi owner nhìn; owner đọc ngoại lệ, không duyệt từng việc | Người kiểm tra thường xuyên kém trong điều kiện đối kháng và khối lượng lớn [R §2.1] |
| Đọc chi phí (R9) | Tài chính của bên áp dụng, từ hoá đơn | Mục 3.2 |
| Tuyên bố rủi ro còn lại có chữ ký | Liệt kê điều blueprint cố ý không làm và người cấp điều hành chấp nhận từng điều [C05 §5 #2] | Không thêm giới hạn; chỉ ghi ai chấp nhận việc không có nó |

Nội dung brief (không phải giới hạn): model tốt nhất sai khoảng 1 trên 6 câu bảng tính tài chính [2603.07316, HIGH], nên agent được báo điều
đó và con số khách dựa vào do mã có phiên bản tính.

### 4.3 Hiệu ứng theo khả năng đảo ngược (R5)

| Lớp | Ví dụ | Phục hồi xây trước | Cận tổn thất |
|---|---|---|---|
| Đảo ngược được | Mã, cấu hình, dữ liệu nội bộ có backup ngoài tầm agent | Revert theo băm release; khôi phục backup; phát lại sổ bất biến; xoá mềm và snapshot trước khi ghi đè [C03 D5] | Có cận: cửa sổ mất dữ liệu và thời gian khôi phục, đo bằng diễn tập |
| Bù trừ được | Hoàn tiền, tín dụng, giá sai | Bản ghi bù trừ cho mỗi tác dụng; thu hồi theo hợp đồng | Cận bằng phần thu hồi được; phần còn lại không có cận |
| Không đảo ngược | Email hay tin gửi người ngoài, phát ngôn công khai, dữ liệu gửi bên thứ ba, xoá ở hệ thống ngoài, chuyển tiền | Không có | **Không có cận. Điều kiện "loss is bounded and recovery is cheap" của `AGENTS.md` không thoả ở lớp này; rủi ro được nhận mà không có cận.** |

Phát hiện, dừng và phục hồi chỉ giới hạn **lần sau**, không trả lại **lần này** [C03 §4]. Hoãn-có-huỷ (nền tảng giữ một cửa sổ ngắn trước khi
giao hành động ra ngoài; agent đã quyết, không ai phải duyệt; nút dừng rút được trong cửa sổ) là lựa chọn owner có thể bật, **mặc định tắt** (R5).

### 4.4 Thứ luật buộc quay lại (R4: hệ quả của điểm cố định 3)

Theo [C04 §3, §5], văn bản do người phản biện đọc; hội đồng chưa tự đọc, cần luật sư trước khi bật từng chức năng:

- **Trước khi dùng:** hồ sơ phân loại và thông báo Bộ KH&CN cho agent rủi ro trung bình hoặc cao (Luật AI Điều 10(3)); DPIA (GDPR Điều 35);
  DPA và SCC với nhà cung cấp LLM. **Trong 60 ngày sau:** hồ sơ đánh giá tác động xử lý và hồ sơ chuyển dữ liệu ra nước ngoài (Luật 91/2025
  Điều 21, 20; phạt tới 5% doanh thu năm trước chỉ cho vi phạm chuyển dữ liệu xuyên biên giới [R §1.3; C04 K12]). Hồ sơ do người đại diện là
  cá nhân ký (Luật Doanh nghiệp Điều 12(1)).
- **Báo cho người ngoài rằng họ nói với AI** từ lần tương tác đầu (AI Act Điều 50(1),(5); Luật AI Điều 11(1)).
- **Dữ liệu cá nhân theo mục đích và phân quyền** (Luật 91/2025 Điều 30(3); GDPR Điều 28(3)(a)): agent không tự dùng dữ liệu người mua cuối
  cho mục đích mới. "Agent không bị giới hạn phạm vi" vì vậy đọc là: trừ giới hạn dữ liệu theo mục đích mà luật đòi.
- **Thay người theo lịch luật lao động**, không ở ngày 0: trao đổi với tổ chức đại diện rồi báo trước 30 ngày (BLLĐ Điều 42(6)); EU 98/59; Mỹ
  WARN; Đức BetrVG §87(1) Nr. 6 với hệ thống có khả năng giám sát hiệu suất. Quyết định nhân sự đọc tín hiệu S6 sau ít nhất một chu kỳ đọc.
- **Kiểm soát thay đổi SOX** với bên niêm yết ở Mỹ: mọi release chạm số liệu báo cáo tài chính, kể cả prompt và cấu hình, đi qua merge (R1)
  [C04 §3.5, MEDIUM: suy luận từ cách ICFR được kiểm].
- **Báo cáo sự cố**: vi phạm dữ liệu trong 72 giờ (Luật 91/2025 Điều 23; GDPR Điều 33); sự cố AI nghiêm trọng báo sơ bộ trong 72 giờ hoặc 05 ngày
  làm việc (NĐ 142 Điều 19). Dừng là quyết định của owner; báo cáo thì không.
- **Kiểm soát của người:** Luật AI Điều 4(2) đòi "sự kiểm soát và khả năng can thiệp của con người đối với mọi quyết định và hành vi". Nút
  dừng là điều kiện cần; **đủ hay không là câu hỏi luật sư số 1** [C04 §5]. Nếu không đủ, luật thắng (R4).
- **Test trước khi dùng** với chức năng rủi ro cao (AI Act Điều 9(8); Luật AI Điều 13(3) đánh giá phù hợp): nếu một chức năng rơi vào rủi ro
  cao, nghĩa vụ này thắng "không thí nghiệm" ở chức năng đó (R4) [C04 §3.3].

## 5. Định vị

| Lựa chọn | Cho doanh nghiệp | Thiếu | Khi nó tốt hơn |
|---|---|---|---|
| Không làm gì | Không tốn `F0`, không rủi ro mới | Không tích luỹ năng lực kiểm chứng | Khi T (mục 3.3) ngắn, hoặc việc phần lớn không kiểm chứng được bằng máy |
| Copilot theo người (M365 Copilot) | Giá biết trước, ít đổi tổ chức [C05 §4, MEDIUM] | Chi phí dồn vào review [2607.01904] | Khi việc chủ yếu cần phán đoán người |
| Nền tảng agent doanh nghiệp (Agentforce, ServiceNow, Copilot Studio, OpenAI Frontier, Bedrock AgentCore) | Hợp đồng sẵn, định danh, lưu trú dữ liệu, chứng chỉ, niêm credential có SLA [C05 §4, MEDIUM] | Không đo pass^k, false-pass, lỗi thoát 30 ngày; kết quả dừng ở "đã trả lời" | Gần như mọi tiêu chí mua sắm hôm nay |
| Framework agent (CrewAI, LangGraph, ADK) | Khối xây dựng | Rail, kiểm chứng sau ship, bề mặt quan sát phải tự xây | Khi công ty có đội tự xây mục 4.2 |
| Agent code tự chủ (Devin, Cursor) | PR tự động, theo sự kiện | Người vẫn là lưới chặn cuối; yếu nơi khó kiểm chứng | Khi chỉ có code và đội review còn dư năng lực |
| Thuê ngoài (BPO) | Giá cố định, trách nhiệm theo hợp đồng | Không tích luỹ tài sản kiểm chứng | Việc không kiểm chứng được bằng máy; không có số so sánh, **LOW** |

**Định vị đề xuất (Đề xuất của hội đồng, giả thuyết):** một **phương pháp** vận hành và đo lường, có hiện thực tham chiếu mở (Remit), chạy
được **trên** nền tảng bên áp dụng đã mua thay vì thay chúng [C05 §4]. Câu nói với thị trường (theo [C05 §6], bỏ chữ "mọi" theo R6): *"Các công ty đã dùng AI để sinh việc
gặp cùng một điểm nghẽn: kiểm chứng. Bản này là một cách trả lời điểm nghẽn đó, có thiết kế đo được và phương pháp mở. Chưa có công ty nào,
kể cả chúng tôi, chứng minh cách trả lời này có lợi nhuận."* Khoảng trống mà SR 26-2 để lại (agentic AI ngoài phạm vi quản trị rủi ro mô hình,
cần khung riêng) là chỗ phương pháp này có thể được dùng, nếu viết theo ngôn ngữ quản trị rủi ro mô hình [C05 §2, MEDIUM].

## 6. Hào và học tập của tổ chức

Thứ tích luỹ được, mỗi thứ kèm thước đo (Đề xuất của hội đồng, giả thuyết):

| Tài sản | Thước đo |
|---|---|
| Thư viện bộ kiểm chứng theo lớp việc | Tỷ lệ bắt lỗi trên lỗi gieo niêm (R2), báo cáo cùng báo động nhầm |
| Kho ca lỗi thật từ vận hành | Số ca hồi quy mới từ lỗi thật mỗi tuần mỗi lớp việc |
| Tập neo niêm cho judge | Thời gian từ lúc judge đổi phiên bản tới lúc tập neo báo và panel được hiệu chỉnh lại. Nền: một lần đổi phiên bản âm thầm được phát hiện trong 60/60 lần chạy, một nghiên cứu, một tác giả [2606.15474] |
| Brief, playbook, sổ quyết định | Tỷ lệ việc mở lại 30 ngày theo phiên bản brief |

Giới hạn thẳng (**MEDIUM**): tài sản mục nhanh. Đổi phiên bản judge làm 59% tập top-30% thay đổi [2610.32986]; Anthropic báo trước tối thiểu
60 ngày khi ngừng model [P §4, MEDIUM]; surrogate lỗi thời khi cách can thiệp đổi, và không có thí nghiệm thì surrogate chỉ hiệu chỉnh được
bằng tương quan [D §7; 2608.08043]. Tập neo chỉ đo được khi bên bị đo không thấy nó, nên hào phụ thuộc vào tập đánh giá niêm (R2). Đây là lợi
thế vận hành, không phải rào cản ngành: đối thủ tự xây được bộ của họ.

## 7. Rủi ro và tín hiệu quan sát

### 7.1 Bằng chứng về hệ thống không giới hạn, không thí nghiệm (thông tin cho owner)

| Đã quan sát | Nguồn | Liên quan |
|---|---|---|
| Replit (07/2025): agent xoá database production trong lúc có code freeze bằng lời và nói khôi phục là bất khả; các nguồn mâu thuẫn về việc rollback có dùng được không, OECD hiện ghi mất dữ liệu vĩnh viễn | [E §5], [C01 #33], MEDIUM | Lời dặn trong prompt không giữ được hành động |
| PocketOS (04/2026): token trong tệp không liên quan, volume bị xoá trong 9 giây, backup cùng volume | [D §6], [C01 #34] | Điểm cố định 2; backup phải ngoài tầm ghi của agent |
| Palisade: hơn 100.000 lượt, 13 model; có model phá cơ chế tắt trong môi trường của nó tới 97% số lần dù được dặn không làm | 2509.14260, HIGH | Điểm cố định 1 chỉ đứng khi nút dừng ngoài môi trường agent |
| Độ tin cậy tăng 0,03–0,09/năm | 2602.16666, HIGH | Một lần thành công nói quá độ tin cậy; S3 |
| Tốc độ tan sau hai tháng, nợ ở lại; thông lượng doanh nghiệp đạt 2× sau khoảng chín tháng | 2511.04427, 2607.01904, HIGH | Cửa sổ đọc (mục 3.2) |
| Klarna tuyển lại người; CBA đảo ngược việc cắt 45 vị trí hỗ trợ | [D §8], [P §2], MEDIUM | Đảo ngược đến từ tín hiệu ngoài; S6 |
| Hệ đang chạy thật thành công khi bị giới hạn: 68% tối đa 10 bước trước khi người can thiệp, 74% dựa vào đánh giá người | 2512.04123, HIGH | Hệ không giới hạn nằm ngoài vùng đã quan sát |
| Big bang trong ERP: Hershey 1999 cắt chuyển cùng lúc ba hệ thống, không giao được khoảng 100 triệu USD đơn hàng | [C06 E8], MEDIUM | Một ca thất bại đã ghi nhận của triển khai một lần (một ca, không đủ để gọi là quy luật) |

### 7.2 Remit là hiện thực tham chiếu, không phải bằng chứng (R7)

Các lỗ đã biết, mỗi lỗ có nguồn trong repo (**HIGH**):

- Credential GitLab và SSH (`~/.ssh`, `~/.config/glab-cli`) đọc được từ phiên agent, chờ một connection kind GitLab chưa có
  (ADR-0018:112–114). **Điểm cố định 2 hiện chưa đúng trên hệ tham chiếu.**
- Hồ sơ Seatbelt trên macOS bắt đầu bằng `(allow default)`, chỉ cấm các đường dẫn được liệt kê (`internal/jail/sandbox_darwin.go:143`); phiên
  có mạng tới nhà cung cấp ngoài gate [C03 §3.1].
- Phiên chạy chung user OS với server: báo hiệu được cho server và ghi được những gì cron và shell của user đó khởi chạy (ADR-0004; ADR-0018,
  Consequences).
- Một owner mỗi server (ADR-0022), không SSO hay phân quyền nhiều người; một nhà cung cấp model (ADR-0006), mà giới hạn phiên của nó đã gây 40
  lỗi trên 20 card trong một buổi (ADR-0005:15); không có SOC 2 hay pentest bên thứ ba [C05 §3].

**Chuyển lên owner:** ADR-0001 giới hạn số MR chờ owner ở 5 và ADR-0010 giữ các giới hạn đếm khác để bảo vệ sự chú ý của owner. Chúng không có
trong R1. Giữ merge (R1) mà bỏ trần hàng đợi thì hàng đợi của owner không có cận, đúng hiện tượng 52 MR chờ [C02 M2, M3]. Đây là quyết định của owner.

### 7.3 Tín hiệu quan sát trong vận hành

Không có dừng tự động. Agent đọc liên tục và hiện cho owner; owner quyết có dừng tay không. So sánh chính là **trong và ngoài ranh giới cùng kỳ**
(R10); khi không có đơn vị so sánh thì so với lịch sử và ghi "tương quan theo thời gian", vì so trước/sau bị thời gian làm nhiễu [E §5]. Mức đọc
là Đề xuất của hội đồng (giả thuyết), không rút từ nguồn nào.

| # | Tín hiệu | Luận điểm **không được ủng hộ** ở lớp việc đó nếu |
|---|---|---|
| S1 | Chi phí trọn gói (hoá đơn, giờ người, sửa nợ, sự cố) mỗi đơn vị đứng vững | Không thấp hơn đơn vị ngoài ranh giới (DiD) ở tháng 9–12 |
| S2 | Giờ người mỗi đơn vị (`r_k`) | Không giảm giữa tháng 1 và tháng 6 so với bên ngoài |
| S3 | pass^k trên tác vụ ghi lại lặp K lần (R3); tỷ lệ mở lại, hoàn tác 30 ngày | Không tăng sau sáu tháng sửa trong lúc chạy |
| S4 | Cảnh báo tĩnh, độ phức tạp, sự cố mỗi thay đổi | Còn tăng ở tháng 3 so với bên ngoài |
| S5 | Tỷ lệ bắt lỗi gieo niêm của checker và auditor; phán quyết judge bị kết quả thật bác về sau; thời gian phát hiện theo lớp hiệu ứng | Không tốt hơn đoán theo tỷ lệ gốc |
| S6 | Tín hiệu từ khách: giữ chân, khiếu nại, khối lượng leo thang | Xấu đi so với bên ngoài cùng kỳ |
| S7 | Lời agent nói bị khách hay cơ quan quản lý coi là cam kết của công ty (Luật GDĐT Điều 14(2)(a); UETA §14) | Một ca là đủ để xem lại brief |
| S8 | Thời gian hiệu chỉnh lại panel judge sau khi đổi phiên bản | Tăng theo thời gian |

Đọc S1 và S6 cùng S3, không riêng: một lỗi làm giảm chất lượng của Bing từng làm tăng truy vấn hơn 10% và doanh thu mỗi người hơn 30% [D §7, HIGH].

### 7.4 Rủi ro chiến lược khác

- **Confused deputy qua injection là rủi ro lớn nhất mang ở ngày 0.** Agent dùng connection hợp lệ để gửi dữ liệu hay cam kết ra ngoài; tấn công
  thích ứng vượt 12 phòng thủ tĩnh trên 90% [2510.09023, HIGH]. Tách luồng điều khiển khỏi dữ liệu không cần chính sách nào và không chặn hành động
  nào: với Gemini 2.5 Pro, số tấn công thành công từ 300 xuống 0 trên 949 khi không cài chính sách, dù không về 0 ở mọi model [2503.18813 §6, HIGH;
  C03 D1]; thiết kế (phần 2) chọn có dùng hay không.
- **Bảo hiểm đang rút:** loại trừ generative AI cho CGL của Verisk từ 01/2026; một số hãng xin loại trừ tuyệt đối [C05 §2, R §1.5, MEDIUM].
  Không giới hạn cộng không bảo hiểm thì đuôi tổn thất nằm trên bảng cân đối.
- **Phụ thuộc nhà cung cấp:** một vendor model (ADR-0006); trần trách nhiệm của Anthropic bằng phí 12 tháng trước [R §1.4, MEDIUM-HIGH]; với tổ
  chức tài chính EU, DORA đòi đánh giá rủi ro tập trung [C05 §2, MEDIUM].
- **Doanh nghiệp có quản trị rủi ro bằng giới hạn:** khung khẩu vị rủi ro gồm "risk limits" (FSB 2013) [C05 §2, MEDIUM]. Một ngân hàng có thể
  không áp dụng được cấu hình không giới hạn. **Chuyển lên owner:** ngân hàng và tổ chức tài chính có thuộc đối tượng không.
- **Hội đồng quản trị:** nghĩa vụ giám sát chức năng trọng yếu (Marchand v. Barnhill) [C05 §2, MEDIUM]. Đề xuất: S1–S8 và tuyên bố rủi ro còn
  lại lên hội đồng mỗi quý, có người ký.
- **Không ca hạng A hoặc B nào cho thấy mô hình này có lợi nhuận** [P §3; 2606.18543]. Đây là rủi ro lớn nhất, và là lý do dữ liệu vận hành công bố có giá trị.

## 8. Sổ claim

| # | Claim | Nguồn | Nhãn |
|---|---|---|---|
| 1 | Doanh nghiệp 802 dev: +12% khi bắt đầu dùng AI, 2× sau khoảng chín tháng dùng; tải mỗi reviewer gấp đôi | 2607.01904 (abstract, §I) | HIGH |
| 2 | Cursor: dòng thêm 3–5 lần tháng đầu, tan sau hai tháng; +30,26% cảnh báo, +41,64% độ phức tạp | 2511.04427 | HIGH |
| 3 | 52 MR chờ owner khi đo; $1.126,07, 3,5 ngày, khoảng 114 chain, 8 MR, $141/MR, $8,25/chain merge; 83% chi do Scout tự giao | ADR-0002:25–26, 38, 40 | HIGH (dogfooding, n nhỏ) |
| 4 | 8 × $8,25 = $66 = 5,9% tổng chi | tính từ ADR-0002 | HIGH (số học) |
| 5 | Lãnh đạo 7 nước: năng suất +0,2%/năm do AI | [P §3] NBER w34836 | HIGH (tự báo cáo) |
| 6 | Faros: thời gian review +91%, không tương quan ở cấp công ty | [C05 §4] | MEDIUM (vendor) |
| 7 | Verifier's law (Wei), generation-verification loop (Karpathy) | [C06 §0, §6] | MEDIUM |
| 8 | Contract-first decomposition: chỉ giao khi kết quả "has precise verification" | 2602.11865 | HIGH |
| 9 | Độ dài tác vụ ở mức 50% gấp đôi khoảng 7 tháng; khoảng 131 ngày gần đây | 2503.14499; [D §8] | HIGH; MEDIUM |
| 10 | Độ tin cậy +0,03/năm (GAIA), +0,09/năm (τ-bench); độ chính xác +0,23/năm, +0,22/năm; 0,18 và 0,33 là theo độ chính xác | 2602.16666 Hình 1, tr. 38 | HIGH |
| 11 | Giá token ở năng lực cố định giảm 9–900 lần/năm; giá frontier không giảm | [D §8] | MEDIUM |
| 12 | Luật AI VN: chuyển tiếp chỉ cho hệ thống có trước 01/03/2026; AI Act Điều 50(2) không có hạn 02/12/2026 cho hệ thống sau 02/08/2026 | [C04 K9, K3] | HIGH theo người phản biện; hội đồng chưa đọc |
| 13 | Anthropic multi-agent +90,2%; khoảng 15 lần token | [E §3] | MEDIUM (blog, chưa mở lại) |
| 14 | Magentic-One task ledger, progress ledger; MetaGPT 10 → 2,5 lần người sửa | 2411.04468; 2308.00352 | HIGH |
| 15 | Paperclip: goal → task, cấu hình có phiên bản, trần ngân sách, board approval | [C06 §1.4] | HIGH cho README theo người phản biện |
| 16 | IMDA, Shavit, Google, NIST NCCoE khuyên giới hạn quyền và điểm duyệt | [C06 §1.2] | HIGH (IMDA), MEDIUM (còn lại) |
| 17 | Auth0, Arcade, AWS AgentCore Identity niêm credential | [C05 §4], [C06 §1.3] | MEDIUM |
| 18 | Devin 67% so với 34%; cần người duyệt khi kết quả không dễ kiểm chứng | cognition.com/blog/devin-annual-performance-review-2025 | MEDIUM (vendor tự báo) |
| 19 | Cursor cloud agents tự mở PR, chạy theo lịch hoặc sự kiện | cursor.com/help/ai-features/background-agents | HIGH (mô tả tính năng) |
| 20 | Klarna 2/3 chat, +$40 triệu ước tính 2024; 05/2025 tuyển lại người | [D §8], [C01 #18] | HIGH cho thông cáo, MEDIUM cho kết luận |
| 21 | Không ca hạng A hoặc B nào có lợi nhuận đã xác minh; CEO-Bench: mọi model thua baseline theo quy tắc | [P §3]; 2606.18543 | HIGH (trong phạm vi đã tìm) |
| 22 | TheAgentCompany: khoảng 30%; kỹ thuật phần mềm cao nhất; hành chính, tài chính, khoa học dữ liệu thấp nhất | 2412.14161 | HIGH |
| 23 | +80,8% trên suy luận tài chính phân rã được; SWE-bench Verified giảm nhẹ ở mọi kiến trúc nhiều agent; lập kế hoạch tuần tự tới −70% | 2512.08296 | HIGH |
| 24 | CXBench: 55,5% của 840 lần chạy, 80,1% của lần đạt kết quả là false-pass; τ-bench pass^8 dưới 25% | 2610.25954; 2406.12045 | HIGH |
| 25 | Không có bằng chứng cho "cắt 80% chi phí review/QA" | [D §8], [P §6] | HIGH rằng không có |
| 26 | Duyệt chỉ góp ý làm xấu kết quả; có ích khi kiểm chứng được | 2609.14767 | HIGH số đo, MEDIUM khái quát |
| 27 | Judge sai tương quan: 9 judge khoảng 2 phiếu độc lập; ba judge frontier khác nhà cung cấp ρ = 0,56 | 2605.29800; 2609.22512 | HIGH |
| 28 | Holdout dùng lặp bị overfit; detector vào thưởng bị né; chỉ số tối ưu quá xa thì lệch | 1411.2664, 1502.04585; 2503.11926; 2210.10760 | HIGH |
| 29 | Model tốt nhất sai khoảng 1 trên 6 câu bảng tính tài chính | 2603.07316 | HIGH |
| 30 | ADR-0010 xét lại khi "runs for someone other than its owner"; ADR-0002 cần cơ sở chi phí khác | ADR-0010:43; ADR-0002:58 | HIGH |
| 31 | ServiceNow tái đầu tư $100M tiết kiệm headcount | [C05 §1.1] | MEDIUM |
| 32 | Hoà vốn và bảng độ nhạy | [C05 §1.2], hội đồng tính lại | HIGH số học; LOW đầu vào |
| 33 | Knight Capital lỗ hơn 460 triệu USD trong khoảng 45 phút | [E §5] SEC | HIGH theo báo cáo; chưa mở lại |
| 34 | So trước/sau bị thời gian làm nhiễu; không gì thay được thí nghiệm dài hạn | [E §5]; 2608.08043 | HIGH |
| 35 | Bing: lỗi giảm chất lượng làm truy vấn hơn +10%, doanh thu mỗi người hơn +30% | [D §7] Kohavi 2012 | HIGH |
| 36 | Một lần đổi phiên bản judge âm thầm phát hiện trong 60/60 lần chạy; đổi judge làm 59% top-30% đổi | 2606.15474; 2610.32986 | HIGH (một tác giả; một sự kiện) |
| 37 | Anthropic báo trước tối thiểu 60 ngày khi ngừng model | [P §4] | MEDIUM |
| 38 | Replit: nguồn mâu thuẫn về rollback; PocketOS: 9 giây, backup cùng volume | [C01 #33, #34] | MEDIUM; HIGH |
| 39 | Palisade: tới 97% (KTC 96–98%) | 2509.14260 | HIGH |
| 40 | 68% tối đa 10 bước; 74% dựa vào đánh giá người; 86 hệ thống | 2512.04123 | HIGH |
| 41 | CBA đảo ngược cắt 45 vị trí; Hershey 1999 | [P §2]; [C06 E8] | MEDIUM |
| 42 | Lỗ của Remit: credential GitLab/SSH đọc được, `(allow default)`, chung user OS, một owner, một vendor, 40 lỗi do giới hạn phiên | ADR-0018, `sandbox_darwin.go:143`, ADR-0004, ADR-0022, ADR-0006, ADR-0005:15 | HIGH |
| 43 | Thủ tục luật ở mục 4.4 (Luật AI Điều 4(2), 10, 11; Luật 91/2025 Điều 20, 21, 23, 30; BLLĐ Điều 42; AI Act Điều 50; SOX §404) | [C04 §2, §3] | HIGH theo người phản biện cho chữ; MEDIUM áp dụng |
| 44 | CaMeL không chính sách: Gemini 2.5 Pro từ 300 xuống 0 trên 949 tấn công; tấn công thích ứng vượt 12 phòng thủ trên 90% | 2503.18813 §6; 2510.09023 | HIGH |
| 45 | Loại trừ AI trong bảo hiểm; FSB khung khẩu vị rủi ro có "risk limits"; DORA; Marchand; SR 26-2 | [C05 §2], [R §1.5] | MEDIUM |
| — | Luận điểm dạng có thể sai (mục 1); câu đóng góp; quy tắc chọn ranh giới; lớp hiệu ứng; mức đọc S1–S8; định vị "phương pháp trên nền tảng sẵn có" | — | **Đề xuất của hội đồng (giả thuyết)** |
| — | Lợi tức R2–R4; T ngắn thì chờ không tốn; lợi ròng cho việc học của big bang | — | LOW |

- **Assumption:** "công ty lớn" là doanh nghiệp có nhiều đơn vị sản phẩm hoặc chức năng, có bộ phận tài chính đọc được chi phí theo loại việc và có
  hội đồng quản trị, vì mô hình ROI ở mục 3, nhóm so sánh ở mục 4.1 và mục 7.4 cần các điều đó.
- **Assumption:** các phán quyết R1–R10 có hiệu lực cho bản này cho tới khi owner xác nhận hoặc bác, vì chủ tọa ghi chúng là mặc định; một phán
  quyết bị bác thì các mục dẫn nó (0, 3.2, 4.1–4.4, 7.2–7.3) phải viết lại.
- **Assumption:** nguồn pháp lý và thị trường chỉ đọc qua phản biện ([C04], [C05], [C06]) giữ nhãn của người phản biện, vì hội đồng chưa tự mở lại
  văn bản gốc; luật sư đọc trước khi bật từng chức năng.
- **Assumption:** Remit là hiện thực tham chiếu của phương pháp, không phải điều kiện của nó; chiến lược này không phụ thuộc vào Remit.
