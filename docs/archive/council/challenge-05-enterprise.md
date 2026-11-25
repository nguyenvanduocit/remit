# Thách thức 05: Doanh nghiệp lớn có mua không, và kinh tế có đứng không

> Người phản biện về áp dụng doanh nghiệp và kinh tế của hội đồng. Ngày 2026-11-01. Đọc như CFO, CIO, CRO, kiểm toán nội bộ và
> mua sắm. Đọc các bản nháp `01-strategy.md`, `02-design.md`, `03-ledger.md`, `04-launch.md`, ADR-0001/0002/0004/0005/0006/0010/0018/0022.
> Không sửa bản nháp nào. Nhãn: **HIGH** (đã đọc nguồn gốc trong lần này), **MEDIUM** (nguồn thứ cấp, vendor, hoặc kết quả tìm kiếm
> chưa mở trang gốc), **LOW** (suy luận). Quyết định của owner (bật một lần, không thí nghiệm, mọi thứ mở, không giới hạn sáng kiến,
> ba điểm cố định) là đầu vào; bài này không tranh luận lại nó, mà nói nó tốn gì khi người mua là một doanh nghiệp có hội đồng quản trị.

## 0. Kết luận trước

**Như đang viết, một doanh nghiệp lớn không áp dụng bản này.** Lý do không phải thiết kế kém; phần kỹ thuật (log do nền tảng ghi,
credential niêm, scorecard pass^k) tốt hơn phần lớn những gì vendor đang bán. Lý do là năm chỗ va trực tiếp với cách một doanh nghiệp
có người được uỷ thác (fiduciary) ra quyết định:

1. **Không giới hạn** va với khung khẩu vị rủi ro, vốn định nghĩa bằng giới hạn (FSB 2013, mục 2).
2. **Không thí nghiệm** làm lợi tức không quy được nguyên nhân; CFO không ký một khoản đầu tư mà thiết kế cấm đo tác động của nó (mục 1).
3. **Hiện thực tham chiếu là của một người**: một owner, một máy, một nhà cung cấp model, credential GitLab và SSH còn đọc được từ
   phiên agent (ADR-0018:112-114). Không có gì để mua sắm ký hợp đồng (mục 3).
4. **Công bố ra thế giới** (04 §4.2) va với bí mật thương mại, nghĩa vụ công bố thông tin và việc postmortem thành chứng cứ (mục 2).
5. **Đuôi tổn thất không có trần và đang bị bảo hiểm loại trừ** (Verisk CG 40 47 từ 01/2026; W.R. Berkley loại trừ tuyệt đối) (mục 2).

Bản này **khớp với một owner-operator** chịu rủi ro của chính mình: các quyết định của owner nhất quán khi người quyết, người chịu lỗ
và người hưởng lợi là một. Chúng không còn nhất quán khi ba vai đó tách ra, như ở mọi công ty lớn. **HIGH** cho các dữ kiện, **MEDIUM**
cho kết luận.

## 1. Mô hình ROI dựng lại

### 1.1 Công thức

Bản 01 §3.2 đưa ra hàm lợi ích ròng nhưng thiếu năm khoản mà CFO sẽ hỏi đầu tiên. Dựng lại theo kỳ `t`, cho lớp việc `k`:

```
Lợi ích tiền mặt(t) = Σ_k N_k · s_k · a_k · H_k · w · c_k            (giá trị thay thế, quy ra tiền)
                    + ΔDoanh thu(t)                                   (không quy được khi không có đối chứng)
Chi phí(t)          = S(t) + Σ_k N_k · r_k · w + D(t) + E[L](t) + G(t) + F0/H + F(t)
ROI(T)              = Σ_t [Lợi ích − Chi phí] / Σ_t Chi phí,  T ≥ 3 tháng (lợi ích tháng 1–2 tan, 2511.04427)
```

| Biến | Nghĩa | Loại | Nguồn hoặc tình trạng |
|---|---|---|---|
| `S(t)` | Tổng chi cho agent (model, compute) trong kỳ, kể cả chain bỏ dở | Bằng chứng (n nhỏ) | $1.126,07 / 3,5 ngày (ADR-0002) |
| `M(t)` | Số đơn vị được chấp nhận (MR merge) | Bằng chứng (n nhỏ) | 8 trong 3,5 ngày (ADR-0002) |
| `N_k` | Đơn vị agent tự nhận | Quan sát | Không giới hạn theo quyết định owner |
| `s_k` | Tỷ lệ đơn vị **có cầu thật** (một người lẽ ra sẽ làm việc đó) | **Không có nguồn** | 83% chi đo được là việc Remit tự giao (ADR-0002); không suy ra được `s` cho MR |
| `a_k` | Tỷ lệ đứng vững 30 ngày | **Không có nguồn cho hệ này** | Gần nhất: khoảng một nửa PR qua test SWE-bench Verified sẽ không được merge (METR, [P §2], MEDIUM) |
| `H_k` | Giờ người cho một đơn vị tương đương | **Không có nguồn** | Số của chính công ty |
| `w` | Đơn giá giờ có gánh chi phí | Bằng chứng | $120/giờ kỹ sư (Ord, MEDIUM); trung vị lương software developer Mỹ $133.080/năm, khoảng $64/giờ chưa gánh (BLS 05/2024, MEDIUM qua kết quả tìm kiếm) |
| `c_k` | Tỷ lệ giờ được giải phóng **thành tiền** (giảm người, không tuyển bù, hoặc doanh thu) | **Không có nguồn** | Salesforce: không tuyển bù sau khi số case giảm; ServiceNow: tiết kiệm $100M headcount được tái đầu tư, không vào biên lợi nhuận (dưới) |
| `r_k` | Giờ người duyệt mỗi đơn vị | **Không có nguồn theo đơn vị** | Hướng: thời gian review PR +91% (Faros, 10.000+ dev); tải mỗi reviewer gấp đôi (He et al. 2607.01904, HIGH) |
| `D(t)` | Nợ kỹ thuật quy ra tiền | **Chỉ có hướng** | +30,26% cảnh báo tĩnh, +41,64% độ phức tạp, kéo dài (2511.04427, HIGH); chưa ai quy ra tiền |
| `E[L]` | Tổn thất kỳ vọng = tần suất × mức nặng, **trừ phần bảo hiểm trả** | **Không có tần suất** | Mức nặng có đuôi (Knight $460M+/45 phút, [E §5]); phần bảo hiểm trả đang giảm (mục 2) |
| `G(t)` | Chi phí quản trị: DPIA, phân loại rủi ro, kiểm toán, tham vấn người lao động, hồ sơ chuyển dữ liệu | **Bản 01 bỏ sót** | Không có số; bắt buộc theo P3 |
| `F0`, `H`, `F(t)` | Dựng, khấu hao, sửa khi chạy | Giả định | Bản 01 §3.2 |

Hai sửa so với bản 01: (a) **`c_k`**: giờ tiết kiệm không phải tiền tiết kiệm. ServiceNow, theo CFO Gina Mastantuono, đạt $100M
tiết kiệm headcount năm 2025 nhưng tái đầu tư hết vào tuyển sales và kỹ sư, không vào biên lợi nhuận
([The Register 2025-07-25](https://www.theregister.com/2025/07/25/servicenow_100m_ai_savings/), MEDIUM). (b) **`s_k`**: với sáng
kiến không giới hạn, agent tự chọn việc, nên giá trị của việc không ai yêu cầu là ẩn số, không phải bằng chi phí người tương đương.

### 1.2 Phép tính duy nhất làm được: hoà vốn cho một MR

Từ ADR-0002: chi phí trọn gói **$141 mỗi MR merge**; chi phí riêng của một chain đã merge **$8,25**. Phép nhân cho thấy điều
bản 01 chưa nói: 8 × $8,25 = $66, tức **5,9% tổng chi**; 94% chi phí nằm ở chain không merge hoặc chưa merge (52 MR đang chờ,
ADR-0001). Đơn vị kinh tế không phải "một chain" mà là **năng suất merge của chi tiêu**.

Hoà vốn khi `s · a · H · w ≥ 141 + r · w`, tức `H* = (141 + r·w) / (s · a · w)`:

| Kịch bản | `w` | `r` | `s` | `a` | `H*` (giờ người tương đương mỗi MR để hoà vốn) |
|---|---|---|---|---|---|
| Lạc quan tối đa | $120 | 0 | 1 | 1 | 1,18 |
| Có review nửa giờ | $120 | 0,5 h | 1 | 1 | 1,68 |
| Đơn giá BLS chưa gánh | $64 | 0,5 h | 1 | 1 | 2,70 |
| Nửa số MR có cầu, 90% đứng vững | $120 | 0,5 h | 0,5 | 0,9 | 3,72 |
| Cầu thật 17% (giả định, xem dưới) | $120 | 0,5 h | 0,17 | 0,9 | 10,9 |
| Giá token giảm 10 lần, năng suất merge giữ nguyên | $120 | 0,5 h | 1 | 1 | 0,62 |

`r = 0,5 h`, `s` và `a` là **giả định của người phản biện**, không có nguồn. `s = 0,17` lấy hình dạng từ 17% chi không do hệ tự
giao (ADR-0002); dùng nó làm tỷ lệ MR có cầu là một **phép tương tự, LOW**. $141 có thể là chi phí tương đương API do Claude Code
báo (`total_cost_usd`), không phải tiền mặt khi chạy bằng thuê bao: ADR-0010 ghi trường đó "is not read" nữa, ADR-0002 ghi lý do
xem lại là "subscription versus metered pricing", và ADR-0006 chạy agent bằng đăng nhập Claude Code của máy (**LOW**, suy luận;
cần xác minh cơ sở chi phí trước khi đưa $141 cho CFO). n = 8, một người duyệt, đang dogfood: **không suy rộng được**.

### 1.3 Đầu vào nào lật dấu

| Đầu vào | Hướng | Có lật dấu không | Vì sao |
|---|---|---|---|
| Năng lực review `R` (merge/kỳ) | Quyết định | **Có, mạnh nhất** | Khi `N·p > R`, chi phí mỗi merge = `S/R`, tăng tuyến tính theo `S`. Không giới hạn sáng kiến cộng rail merge (04 §0) nghĩa là chi phí mỗi đơn vị không có trần. Dữ kiện: 52 MR chờ (ADR-0001), 94% chi ngoài chain merge |
| `s` (tỷ lệ việc có cầu) | Quyết định | **Có** | `H*` tăng 1/s; từ 1,68 h lên 10,9 h giữa `s = 1` và `s = 0,17` |
| `c` (giờ thành tiền) | Quyết định | **Có** | `c = 0` thì lợi ích tiền mặt bằng 0 dù năng suất tăng (ServiceNow tái đầu tư; Klarna tuyển lại người, [Entrepreneur dẫn Bloomberg](https://www.entrepreneur.com/business-news/klarna-ceo-reverses-course-by-hiring-more-humans-not-ai/491396), MEDIUM) |
| `E[L]` sau bảo hiểm | Đuôi | **Có, ở một sự kiện** | Một Knight Capital xoá lợi ích nhiều năm; bảo hiểm đang rút (mục 2) |
| `D` | Chậm | Có thể, sau tháng 3 | Nợ kéo dài, tốc độ tan sau hai tháng (2511.04427, HIGH) |
| `a` | Trung bình | Có | Chia trực tiếp `H*` |
| Giá token `g` | Thuận | **Không đủ một mình** | Năng lực cố định: giảm 9–900 lần/năm, trung vị 50 lần ([Epoch](https://epoch.ai/data-insights/llm-inference-price-trends), HIGH cho Epoch). Nhưng gần trần năng lực, chi phí giờ tăng theo độ dài tác vụ: GPT-5 khoảng $13/h cho tác vụ 45 phút, khoảng $120/h cho tác vụ 2 giờ; o3 $350/h ([Ord](https://www.tobyord.com/writing/hourly-costs-for-ai-agents), MEDIUM). Giá giảm không sửa được `R`, `s`, `c` |

**Kết luận về ROI (HIGH cho cấu trúc, LOW cho mọi con số):** ROI không tính được cho bất kỳ lớp việc nào hôm nay. Dấu của nó do
`R`, `s` và `c` quyết định, không do giá token. Cả ba đều nằm trong tay doanh nghiệp, không trong tay agent, và cả ba đều chưa có
số đo trong bộ nguồn.

### 1.4 Đo được gì mà không phải thí nghiệm

Bản 04 §5.1 đặt ranh giới: "bên ngoài ranh giới, không gì thay đổi". Các đơn vị bên ngoài là **nhóm so sánh tự nhiên**. So chênh lệch
trong chênh lệch (DiD) giữa đơn vị trong và ngoài ranh giới, theo cách He et al. và He (Cursor) làm với áp dụng so le, là phân tích quan
sát, không phải thí nghiệm: không phân nhóm ngẫu nhiên, không holdout trên khách. Đây là cách duy nhất trong khung của owner cho CFO
một con số gần nhân quả. **Đề xuất của người phản biện (giả thuyết)**; hội đồng cần owner xác nhận nó nằm trong "không thí nghiệm".

## 2. Phản đối của từng uỷ ban

Phân loại: **E** = blueprint trả lời bằng bằng chứng; **A** = trả lời bằng giả định; **N** = không trả lời.

| Bên | Câu họ sẽ hỏi | Blueprint | Đánh giá |
|---|---|---|---|
| **Uỷ ban rủi ro** | Khẩu vị rủi ro và giới hạn là gì? | Không giới hạn; tổn thất bị chặn bởi thời gian phát hiện, dừng, phục hồi (02 §0) | **N.** FSB định nghĩa khung khẩu vị rủi ro gồm "a risk appetite statement, risk limits, and an outline of the roles" ([FSB 2013](https://fsb.org/2013/11/r_131118), MEDIUM qua kết quả tìm kiếm). Một ngân hàng không có lựa chọn ký thiếu giới hạn |
| Uỷ ban rủi ro | Rủi ro tập trung nhà cung cấp? | 04 §5.2 có "nhà cung cấp dự phòng" | **N, mâu thuẫn nội bộ.** ADR-0006 chọn Claude Code là agent duy nhất và bác cả abstraction provider. Với tổ chức tài chính EU, DORA buộc đánh giá rủi ro tập trung trước khi ký và giữ Register of Information từ 17/01/2025 ([FMA Áo](https://www.fma.gv.at/en/?p=158458), MEDIUM) |
| Uỷ ban rủi ro | "Không giới hạn" có thật không? | Không trần chi phí (ADR-0010) | **E, chống lại blueprint.** Rate limit của nhà cung cấp là trần thật: 26/10/2026, 40 lỗi trên 20 card trong 52 phút khi Claude chạm giới hạn phiên (ADR-0005:12-16). Ở quy mô doanh nghiệp, mua thêm hạn mức là một cam kết chi tiêu, tức một trần do mua sắm đặt |
| **Uỷ ban kiểm toán / ICFR** | Ai tách người làm và người kiểm? | Auditor khác pipeline (02 I9), verdict cần `evidence_ref` (I6), merge của owner (F4) | **A.** Luận điểm "agent tự xây bộ kiểm chứng" (01 §1) đặt người làm và người viết phép kiểm cùng một phía. COSO 2026 đưa khái niệm "AI reliance" khi ban điều hành dựa vào đầu ra AI làm bằng chứng kiểm soát, và SEC coi trách nhiệm ICFR mở rộng tới mọi công nghệ trong báo cáo tài chính ([KPMG về COSO](https://kpmg.com/us/en/frv/reference-library/2026/coso-releases-roadmap-internal-control-over-generative-ai.html), MEDIUM). Một owner duyệt mọi merge là phân tách nhiệm vụ của một người |
| ICFR | Số tài chính có đúng không? | Tool tính xác định (04 D9, 02 I10) | **E** cho thiết kế (FinSheet-Bench 82,4%, HIGH). Chưa có kiểm thử hiệu quả kiểm soát nào |
| **Kiểm toán nội bộ** | Có dấu vết kiểm toán không sửa được không? | Log do nền tảng ghi, chỉ thêm (02 I4, 04 D4); băm release (I12) | **E** cho thiết kế; **N** cho chứng thực: chưa có SOC 2, ISAE 3402 hay ánh xạ IIA AI Auditing Framework ([IIA](https://www.theiia.org/en/content/tools/professional/2023/the-iias-updated-ai-auditing-framework/), MEDIUM) |
| **Pháp chế** | Ai chịu lời hứa của agent với khách? | Công ty chịu (Moffatt); cam kết không bị giới hạn bằng mã (02 §2.7) | **N.** Blueprint tự nhận phục hồi không xoá được email hay khoản hoàn tiền (02 §9 điểm 1). Trần trách nhiệm của Anthropic bằng phí 12 tháng ([R §1.4], MEDIUM-HIGH): khoảng chênh thuộc về công ty |
| Pháp chế | Công bố ra thế giới có an toàn không? | Chỉ lọc dữ liệu cá nhân (04 §4.2, D12) | **N.** Không bàn bí mật thương mại, NDA với khách, thông tin trọng yếu với nhà đầu tư, hay việc postmortem công khai thành lời thừa nhận trong tranh chấp. Lọc PII là **giả định**, tự ghi chưa có bằng chứng |
| **An ninh (CISO)** | Prompt injection? | Không Rule of Two, không allowlist, chỉ tripwire (02 §2.8) | **N, có chủ đích.** Blueprint tự nói đây là rủi ro ngày 0 "mang ít nhất" (04 §6 #4). Tấn công thích ứng vượt 12 phòng thủ >90% ([R §3.1], HIGH) |
| An ninh | Credential có thật sự ngoài ngữ cảnh agent? | Điểm cố định 2, ADR-0018 | **N cho hiện thực.** ADR-0018:112-114: credential GitLab và SSH (`~/.ssh`, `~/.config/glab-cli`) "are readable by a session"; phiên agent chạy chung user OS với server (ADR-0004, thu hẹp nhưng chưa đóng). Điểm cố định 2 đang bị vi phạm trên hệ tham chiếu |
| An ninh | Định danh doanh nghiệp (SSO, SCIM, RBAC)? | Google qua Firebase hoặc khoá owner cục bộ (ADR-0022) | **N.** Một owner mỗi server; không có vai trò, không có phân quyền giữa nhiều người |
| **Hội đồng lao động / công đoàn** | Có tham vấn trước không? | "đại diện người lao động được tham vấn trước khi thay đổi vị trí" (04 §5.3) | **A, sai thời điểm.** Ở Đức, §87(1) Nr. 6 BetrVG cho Betriebsrat quyền đồng quyết định bắt buộc với hệ thống kỹ thuật **có khả năng khách quan** giám sát hành vi hay hiệu suất, không cần ý định ([CMS](https://cms.law/de/deu/legal-updates/it-mitbestimmung-und-ki), MEDIUM); "mọi hành động ghi ai đã làm" là đúng loại đó. EU AI Act Đ.26(7): thông báo người lao động và đại diện **trước** khi đưa hệ rủi ro cao vào dùng ([artificialintelligenceact.eu](https://artificialintelligenceact.eu/article/26/), MEDIUM). Ngày 0 do thoả ước lao động quyết, không do owner. CBA đảo ngược cắt 45 vị trí sau áp lực của FSU ([Bloomberg 2025-08-21](https://www.bloomberg.com/news/articles/2025-08-21/commonwealth-bank-reverses-job-cuts-decision-over-ai-chatbots), MEDIUM) |
| **Bảo hiểm** | Có được bảo hiểm không? | "đọc điều khoản loại trừ AI" (04 §5.3, MEDIUM-LOW) | **N.** Từ 01/01/2026, Verisk/ISO có ba endorsement loại trừ generative AI cho CGL, gồm CG 40 47 ([Big I](https://www.independentagent.com/vu_resource/verisk-to-roll-out-new-general-liability-exclusions-for-generative-ai-exposures/), MEDIUM); AIG, Great American, W.R. Berkley xin phép loại trừ trách nhiệm từ chatbot và agent; Berkley đề xuất loại trừ "absolute" cho D&O, E&O ([the-decoder dẫn FT](https://the-decoder.com/major-insurers-seek-to-exclude-ai-related-risks-from-corporate-policies/), MEDIUM). Không giới hạn + không bảo hiểm = đuôi để trần trên bảng cân đối |
| **Hội đồng quản trị** | Hội đồng giám sát thế nào? | Nút dừng; scorecard M1–M7 | **A.** Theo Delaware (Marchand v. Barnhill 2019), hội đồng phải cố gắng thiện chí đặt "a reasonable system of board-level monitoring and reporting" cho chức năng mission-critical ([Harvard Law Forum](https://corpgov.law.harvard.edu/2019/07/06/director-independence-and-oversight-obligation-in-marchand-v-barnhill/), MEDIUM). Scorecard là nguyên liệu tốt; blueprint chưa nói cái gì lên hội đồng, khi nào, ai ký |
| **CFO** | Lợi tức quy được nguyên nhân không? | Không; "tương quan theo thời gian" (01 §3.2) | **E rằng không.** Thẳng thắn, nhưng là lý do từ chối. Mục 1.4 là đường ra |
| **Ngân hàng: quản trị rủi ro mô hình** | Validate mô hình thế nào? | Bộ hồi quy K ≥ 5, tập neo judge | **A.** SR 26-2 (17/04/2026) thay SR 11-7 và đặt generative và agentic AI ngoài phạm vi, cần "separate governance frameworks" ([CRA](https://www.crai.com/insights-events/publications/model-risk-management-guidance-sr-26-2-in-the-era-of-ai/), MEDIUM). Khoảng trống này là cơ hội: blueprint có thể là khung đó, nếu được viết theo ngôn ngữ của MRM |

Đếm: 16 câu; **E** 4 (hai trong đó là bằng chứng chống blueprint: rate limit và ROI không quy được), **A** 4, **N** 9 (câu kiểm toán nội bộ tính cả E cho thiết kế lẫn N cho chứng thực).

## 3. Rào cản áp dụng ở doanh nghiệp lớn

| Rào cản | Thực tế | Bằng chứng |
|---|---|---|
| **Mua sắm** | Không có pháp nhân bán, SLA, bồi hoàn, báo cáo SOC 2, bảng câu hỏi vendor. Hiện thực là một tiến trình trên một máy, sau pm2 (`AGENTS.md`, Tools and runtime), SQLite, một owner | ADR-0022, ADR-0004; **HIGH** |
| **Lưu trú dữ liệu** | API Anthropic trực tiếp có `inference_geo` chỉ "us" hoặc "global"; EU qua Bedrock (Ireland, Stockholm từ 04/2026). ADR-0006 dùng đăng nhập Claude Code của máy, không nói đường Bedrock/Vertex | [Claude docs](https://platform.claude.com/docs/en/build-with-claude/data-residency), [cloudmagazin](https://www.cloudmagazin.com/en/2026/04/22/aws-bedrock-anthropic-api-or-self-hosted-ai-inference/), MEDIUM; Việt Nam: hồ sơ chuyển ra nước ngoài trong 60 ngày (Luật 91/2025/QH15 Đ.20(2), [R §1.3]) |
| **Rủi ro vendor** | Một nhà cung cấp model, không lớp thay thế (ADR-0006 bác abstraction một thành viên) | ADR-0006; Anthropic có ISO/IEC 42001 do Schellman cấp ([Anthropic](https://www.anthropic.com/news/anthropic-achieves-iso-42001-certification-for-responsible-ai), MEDIUM): nhà cung cấp đạt, hệ bao quanh thì chưa |
| **Quản trị thay đổi** | Bật toàn bộ một lần, cả ranh giới. Doanh nghiệp lớn đang rút lui khỏi kế hoạch cắt người | Gartner: tới 2027, 50% tổ chức định cắt mạnh nhân sự CSKH sẽ bỏ kế hoạch ([Gartner 2025-06-10](https://www.gartner.com/en/newsroom/press-releases/2025-06-10-gartner-predicts-50-percent-of-organizations-will-abandon-plans-to-reduce-customer-service-workforce-due-to-ai), MEDIUM); Klarna, CBA |
| **Văn hoá** | Vai của người chuyển từ làm sang đọc; một người đọc mọi kết quả | 52 MR chờ một người (ADR-0001); người duyệt chấp nhận 89% vấn đề do LLM nêu ([R §2], HIGH) |
| **Trách nhiệm trong sơ đồ tổ chức** | Blueprint có một "owner" mỗi cell (02 §0). Công ty 50.000 người cần biết ai ký rủi ro của mỗi cell, ai giữ nút dừng cả ranh giới, ai trả lời cơ quan quản lý | EU AI Act Đ.26(2) buộc bên triển khai giao giám sát cho người có năng lực, đào tạo và thẩm quyền ([presencis](https://presencis.com/regulations/eu-ai-act/conformity/article-26-deployer-obligations), MEDIUM). 04 §5.3 có danh sách vai; chưa có RACI |
| **Thời điểm thị trường** | Phần lớn đang thử, ít người mở rộng | McKinsey 2025: 23% đang mở rộng agent ở đâu đó, không quá 10% ở bất kỳ chức năng nào; 39% báo có tác động EBIT, đa số dưới 5% ([McKinsey](https://www.mckinsey.com/capabilities/quantumblack/our-insights/the-state-of-ai), MEDIUM); Gartner: hơn 40% dự án agentic bị huỷ trước cuối 2027 vì chi phí, giá trị không rõ, kiểm soát rủi ro yếu; chỉ khoảng 130 vendor "agentic" là thật ([newelectronics](https://www.newelectronics.co.uk/content/news/over-40-of-agentic-ai-projects-could-be-cancelled-by-2027), MEDIUM) |

Rào cản lớn nhất không có trong bảng 01 §5: **phe mua là CIO, và CIO mua thứ đã có trong hợp đồng hiện tại** (Microsoft, Salesforce,
ServiceNow, Google, AWS). Đây là suy luận (**LOW**), nhưng mọi đối thủ ở mục 4 đều bán qua hợp đồng nền tảng sẵn có.

## 4. Lựa chọn thay thế mà ban điều hành sẽ so

| Lựa chọn | Dữ kiện có nguồn | Có gì blueprint không có | Blueprint có gì họ không có |
|---|---|---|---|
| **Copilot theo người** | M365 Copilot $30/người/tháng, cam kết năm ([o365hq](https://o365hq.com/blog/microsoft-365-copilot-licensing-and-pricing-2026), MEDIUM). Faros, 10.000+ dev, 1.255 nhóm: +21% task, +98% PR merge, thời gian review +91%, "no significant correlation between AI adoption and improvements at the company level" ([Faros](https://www.faros.ai/blog/ai-software-engineering), MEDIUM, vendor) | Giá biết trước, rủi ro thấp, không đổi tổ chức | Nhắm thẳng vào điểm nghẽn review mà Faros đo được |
| **Salesforce Agentforce** | Flex Credits $0,10 mỗi action từ 05/2025 ([eesel](https://www.eesel.ai/blog/agentforce-pricing), MEDIUM). Nội bộ: Benioff nói giảm hỗ trợ từ 9.000 xuống khoảng 5.000 người, agent xử lý khoảng 50% cuộc trò chuyện; người phát ngôn nói là không tuyển bù khi số case giảm ([The Register](https://www.theregister.com/2025/09/02/salesforce_4000_jobs_ai), MEDIUM, CEO tự báo) | Dữ liệu CRM sẵn, hợp đồng sẵn, giá theo đơn vị | Kết quả không dừng ở "đã trả lời"; đo quy trình (M2) và lỗi thoát (M3) |
| **ServiceNow** | CFO: $100M tiết kiệm headcount 2025, tái đầu tư ([The Register](https://www.theregister.com/2025/07/25/servicenow_100m_ai_savings/), MEDIUM) | Nằm trong ITSM sẵn có | Như trên |
| **Microsoft Copilot Studio / Agent Framework** | Copilot Studio: gói 25.000 Copilot Credits/tháng giá $2.400/năm, vượt thì $0,01 mỗi tin ([o365hq](https://o365hq.com/blog/microsoft-365-copilot-licensing-and-pricing-2026), MEDIUM); Agent Framework 1.0 hợp nhất AutoGen và Semantic Kernel, MIT, 04/2026 ([Visual Studio Magazine](https://visualstudiomagazine.com/Articles/2025/10/01/Semantic-Kernel-AutoGen--Open-Source-Microsoft-Agent-Framework.aspx), MEDIUM) | Entra ID, Purview, lưu trú dữ liệu | Chain giữ kết quả đến khi hiệu ứng được kiểm chứng (ADR-0013) |
| **Google Gemini Enterprise** | GA 09/10/2025, từ $30/người/tháng, cộng phí token và compute agent ([TechCrunch](https://techcrunch.com/2025/10/09/google-ramps-up-its-ai-in-the-workplace-ambitions-with-gemini-enterprise), MEDIUM) | Workspace, vùng dữ liệu | Như trên |
| **OpenAI Frontier** | Ra mắt 05/02/2026: nền tảng quản lý "AI coworkers" có ngữ cảnh chung, phân quyền, quản trị; khách: HP, Intuit, Oracle, State Farm, Uber; giới hạn khách, có kỹ sư triển khai tại chỗ, chưa công bố giá ([OpenAI](https://openai.com/index/introducing-openai-frontier/), [Fortune](https://fortune.com/2026/02/05/openai-frontier-ai-agent-platform-enterprises-challenges-saas-salesforce-workday/), MEDIUM) | Gần nhất về ý tưởng "công ty chạy bằng agent", cộng thương hiệu và kỹ sư tại chỗ | Phương pháp mở, scorecard công bố, kiểm chứng sau ship |
| **AWS Bedrock AgentCore** | GA 13/10/2025; Identity với kho token an toàn, agent hành động thay người dùng qua OAuth; Observability tương thích OTEL ([AWS](https://aws.amazon.com/about-aws/whats-new/2025/10/amazon-bedrock-agentcore-available), MEDIUM) | **Niêm credential** đã là sản phẩm có SLA | Ít hơn bản 01 §5 nghĩ: dòng "framework không có niêm credential" cần sửa |
| **Thuê ngoài (BPO)** | Không có số giá so sánh trong bộ nguồn. Klarna quay lại tuyển người làm hỗ trợ sau khi CEO nói chatbot rẻ hơn nhưng "lower quality" ([Entrepreneur](https://www.entrepreneur.com/business-news/klarna-ceo-reverses-course-by-hiring-more-humans-not-ai/491396), MEDIUM): người, trong nhà hay thuê ngoài, vẫn là phương án dự phòng | Trách nhiệm theo hợp đồng, giá cố định | Tài sản kiểm chứng ở lại công ty (01 §6, LOW) |
| **Không làm gì** | NBER w34836: năng suất +0,2%/năm, gần như không tác động ròng lên việc làm ([P §3], HIGH) | Chi phí cơ hội bằng 0 trong ngắn hạn | Lập luận 01 §3.3: năng lực kiểm chứng không mua được sau (LOW, chưa có số đo) |

Đánh giá thẳng (**MEDIUM**): trên cột 3, mọi vendor thắng blueprint ở mua sắm, định danh, lưu trú dữ liệu, chứng chỉ và trách nhiệm
hợp đồng. Blueprint thắng ở **độ trung thực của phép đo** (pass^k, false-pass, lỗi thoát, Wilson) và ở **kiểm chứng sau ship**.
Đó là lợi thế của một phương pháp, không phải của một sản phẩm. Định vị đáng tin hơn là bán phương pháp chạy **trên** các nền tảng
đó, không thay chúng. Hội đồng nên cân nhắc đổi định vị theo hướng này.

## 5. Cần gì để đáng tin với người mua

| # | Thứ cần | Ai đòi | Hiện có |
|---|---|---|---|
| 1 | **Ma trận kiểm soát**: mỗi điểm cố định và invariant I1–I16 ánh xạ tới NIST AI RMF (GOVERN, MAP, MEASURE, MANAGE) và NIST AI 600-1 (12 rủi ro, hơn 200 hành động, 26/07/2024, [NIST](https://www.nist.gov/publications/artificial-intelligence-risk-management-framework-generative-artificial-intelligence), MEDIUM), ISO/IEC 42001 Annex A, sáu bước COSO GenAI (govern, inventory, assess, design, implement, monitor), EU AI Act Đ.9–15 và 26, OWASP LLM Top 10 | Kiểm toán nội bộ, CRO | Có ánh xạ rời rạc (04 M7 → MANAGE 2.4; 02 I1 → OWASP LLM06). Chưa có ma trận |
| 2 | **Tuyên bố rủi ro còn lại có chữ ký**: liệt kê điều blueprint cố ý không làm (không giới hạn, không Rule of Two, cam kết với khách không bị chặn) và người cấp điều hành chấp nhận từng điều | CRO, hội đồng quản trị | 02 §9 có danh sách điểm yếu; chưa có người ký. Cách này tôn trọng quyết định của owner: không thêm giới hạn, chỉ ghi ai chấp nhận việc không có nó |
| 3 | **Một triển khai tham chiếu có số**: ít nhất một quý M1–M7 công bố, cộng chi phí trên kết quả đã xác minh (M6) từ hoá đơn, cộng DiD với đơn vị ngoài ranh giới (mục 1.4) | CFO | Không có. Ghi chi phí bị gỡ (ADR-0010); ADR-0010 tự ghi xem lại khi "runs for someone other than its owner", tức đúng lúc doanh nghiệp áp dụng |
| 4 | **Đóng lỗ credential của hiện thực tham chiếu** (ADR-0018:112-114), user OS riêng cho phiên (ADR-0004), rồi pentest gate và nút dừng bởi bên thứ ba | CISO | Kế hoạch có; chưa đóng |
| 5 | **Diễn tập dừng và khôi phục có đo** (04 D1, D3, M7) với biên bản | Kiểm toán, cơ quan quản lý (EU AI Act Đ.14(4)(e): nút dừng đưa hệ về trạng thái an toàn, [EC AI Act Service Desk](https://ai-act-service-desk.ec.europa.eu/en/ai-act/article-14), MEDIUM) | Có đặc tả, chưa có kết quả |
| 6 | **Định danh doanh nghiệp**: SSO, nhiều owner theo cell, phân quyền theo vai, ghi ai làm | CIO, kiểm toán | Không (ADR-0022) |
| 7 | **Chính sách công bố có pháp chế duyệt**: phân loại cái gì ra ngoài (phương pháp, định nghĩa scorecard) và cái gì không (postmortem có tên khách, số liệu trọng yếu) | Pháp chế | Chỉ có lọc PII (04 D12) |
| 8 | **Gói hồ sơ cho hội đồng lao động**: mô tả dữ liệu về người được ghi, mục đích, thời hạn lưu, quyền truy cập | Betriebsrat, công đoàn | Không |
| 9 | **Hồ sơ bảo hiểm**: điều khoản loại trừ hiện hành của công ty, câu hỏi cho broker, ước tính phần không được bảo hiểm | CFO, bảo hiểm | Một dòng (04 §5.3) |
| 10 | **Đường đa nhà cung cấp hoặc lý do vì sao không cần** viết theo ngôn ngữ DORA | CRO tài chính | ADR-0006 bác; 04 §5.2 giả định có |
| 11 | **Chứng nhận**: ISO/IEC 42001 cho hệ quản lý AI của bên triển khai; SOC 2 Type II hoặc ISAE 3402 nếu Remit thành dịch vụ | Mua sắm | Không |

## 6. Hai tuyên bố lớn: nói được gì

**"Mọi công ty lớn sẽ cần cái này."** Bằng chứng ủng hộ **một phần hẹp hơn**: mọi công ty đã phát copilot đều gặp cùng điểm nghẽn
review. He et al. đo tải mỗi reviewer gấp đôi (HIGH); Faros đo thời gian review +91% và không có tác động ở cấp công ty (MEDIUM);
Remit đo 52 MR chờ một người (HIGH, n = 1). Bằng chứng **chống** phần rộng: chưa ai chứng minh công ty vận hành bằng agent có lợi
nhuận (không ca A/B nào, [P §3], HIGH); CEO-Bench, mọi model thua baseline theo quy tắc (HIGH); Gartner dự đoán không công ty Fortune 500
nào loại bỏ hoàn toàn CSKH do người tới 2028 ([Gartner 2025-09-10](https://www.gartner.com/en/newsroom/press-releases/2025-09-10-gartner-predicts-none-of-the-fortune-500-companies-will-have-fully-eliminated-human-customer-service-by-2028), MEDIUM, dự đoán).

Đề xuất câu chữ: *"Mọi công ty đã dùng AI để sinh việc đều gặp cùng điểm nghẽn: kiểm chứng. Bản này là một cách trả lời điểm nghẽn
đó, có thiết kế đo được và phương pháp mở. Chưa có công ty nào, kể cả chúng tôi, chứng minh cách trả lời này có lợi nhuận."*

**"Bước tiếp theo của nhân loại."** Không có bằng chứng nào trong bộ nguồn đỡ được câu này, và nó làm hại độ tin của phần có bằng
chứng: một uỷ ban kiểm toán đọc câu đó sẽ đọc phần còn lại như tài liệu bán hàng. Tác động vĩ mô hiện đo được nhỏ (+0,2%/năm, NBER
w34836, HIGH); tác động đo được lên việc làm tập trung ở người 22–25 tuổi, thấp hơn 19% so với đối chứng (Brynjolfsson et al., HIGH,
mô tả). Độ dài tác vụ agent làm được tăng gấp đôi khoảng 7 tháng (2503.14499, HIGH) nhưng độ tin cậy tăng 0,03–0,09/năm (2602.16666, HIGH),
và chi phí giờ gần trần năng lực tăng nhanh (Ord, MEDIUM).

Đề xuất câu chữ: *"Nếu một công ty vận hành bằng agent công bố được lợi nhuận đã xác minh cùng phương pháp và số liệu để người khác
tính lại, đó sẽ là bằng chứng đầu tiên loại này. Bản này được viết để tạo ra bằng chứng đó, dù kết quả tốt hay xấu."* Câu này giữ
niềm tin vào hướng đi (01 §0) mà không hứa điều chưa có.

## 7. Điểm mạnh thật

1. **Chẩn đoán đúng và có bằng chứng độc lập.** "Điểm nghẽn là kiểm chứng, không phải sinh" (01 §1) khớp với ba nguồn không liên quan
   nhau: He et al. (HIGH), Faros (MEDIUM), ADR-0001 (HIGH). Đây là luận điểm một CFO sẽ nhận ra ngay từ dữ liệu của chính công ty mình.
2. **Trung thực hiếm thấy.** Bảng "khẳng định chúng tôi không đưa ra" (03 §5), "ROI không tính được" (01 §3.2), "rủi ro ngày 0 mang ít
   nhất" (04 §6 #4). Kiểm toán nội bộ tin tài liệu tự nêu điểm yếu hơn tài liệu không nêu.
3. **Thước đo tốt hơn thị trường.** pass^k, C_out, false-pass, DER_30, Wilson, sai số theo cụm (04 §3). Vendor báo "tỷ lệ giải quyết"
   không định nghĩa mẫu số (03 §5). Scorecard M1–M7 có thể thành chuẩn công bố mà khoảng trống SR 26-2 để lại.
4. **Rail xác định đúng chỗ.** Dừng ở gateway ngoài môi trường agent (04 D1, đúng bài học Palisade, HIGH); log do nền tảng ghi (I4); băm
   release (I12). Ba thứ này là thứ kiểm toán nội bộ hỏi đầu tiên, và đều kiểm được.
5. **Pháp lý có ngày.** 03 §3 và 04 §5.3 có ngày hiệu lực cụ thể cho EU, Mỹ, Việt Nam: đủ để pháp chế bắt đầu, không phải bắt đầu lại.
6. **Tách được thành sản phẩm cho người mua lớn mà không đổi quyết định của owner**: phương pháp đo và kiểm chứng chạy trên nền tảng
   họ đã mua (mục 4). Owner giữ hệ không giới hạn cho công ty của mình; doanh nghiệp lớn mua phần đo được.

## Giả định

- *Assumption: "doanh nghiệp lớn" có hội đồng quản trị, uỷ ban kiểm toán và bảo hiểm D&O, vì các phản đối ở mục 2 đến từ các vai đó.*
- *Assumption: $141/MR trong ADR-0002 là `total_cost_usd` do Claude Code báo, vì ADR-0010 nói chính trường đó không còn được đọc; chưa
  xác minh với owner.*
- *Assumption: DiD với đơn vị ngoài ranh giới không phải "thí nghiệm" theo nghĩa của owner, vì không có phân nhóm ngẫu nhiên hay holdout
  trên khách; cần owner xác nhận.*
- *Assumption: các con số `r = 0,5 h`, `s`, `a` ở mục 1.2 là minh hoạ của người phản biện, không phải ước tính.*

## Đầu vào ROI không có nguồn

`s_k` (tỷ lệ việc agent tự nhận có cầu thật), `a_k` (tỷ lệ đứng vững 30 ngày) cho hệ này, `H_k` (giờ người tương đương), `r_k` (giờ duyệt
mỗi đơn vị; chỉ có hướng), `c_k` (tỷ lệ giờ thành tiền), `D` quy ra tiền, tần suất trong `E[L]` và phần bảo hiểm trả, `G` (chi phí quản trị),
`F0`, cơ sở chi phí của $141 (API hay thuê bao), và `ΔDoanh thu` (không quy được khi không có đối chứng).
