# Bằng chứng còn lại và thiết kế pilot

> Phần bốn của nghiên cứu về hệ thống, sau [`research-ai-autonomous-system-evidence.md`](research-ai-autonomous-system-evidence.md),
> [`research-ai-autonomous-system-design.md`](research-ai-autonomous-system-design.md) và
> [`research-ai-autonomous-system-risk.md`](research-ai-autonomous-system-risk.md). Báo cáo này lấp các khoảng trống còn lại
> (role ngoài code, bằng chứng ngược chiều, công ty do agent vận hành, vận hành, phương pháp thí nghiệm) và kết thúc bằng thiết kế
> pilot. Ngày: 2026-11-01.
>
> Nhãn tin cậy: **HIGH** (tôi đã tải bản toàn văn hoặc mở nguồn gốc và đối chiếu số liệu), **MEDIUM** (sub-agent đọc nguồn gốc
> nhưng tôi chưa đối chiếu, hoặc nguồn thứ cấp hoặc của vendor), **LOW** (snippet, blog, suy luận).

## Cách đọc theo lập trường của dự án

`AGENTS.md` ghi: chúng ta nhận rủi ro khi tổn thất có giới hạn và phục hồi rẻ, đi trước thời đại, và một khó khăn là việc cần làm.
Nên phần bằng chứng ngược chiều dưới đây là **danh sách việc phải xây vào pilot**, không phải lý do dừng. Lập trường cũng nói nghi ngờ dành
cho dữ kiện; ở các mục có số liệu, báo cáo này giữ nguyên tắc đó và ghi rõ chỗ nào đã kiểm, chỗ nào chưa.

Đối chiếu: năm sub-agent nghiên cứu độc lập; tôi tải và tra số liệu trong toàn văn của 28 tài liệu (25 paper arXiv, một bài NBER, một bài Stanford, thư của House
Oversight) và mở lại hai bài hậu kiểm của Anthropic. Nguồn không phải paper (blog, trang vendor, báo chí) do sub-agent đọc qua công cụ tóm tắt
trang nên giữ nhãn MEDIUM hoặc thấp hơn.

## 1. Role ngoài code

Xếp hạng độ chín (nhận định của tôi, MEDIUM): marketing creative > hỗ trợ khách > thiết kế > số liệu tài chính > quản lý sản phẩm.
Hạng thấp của quản lý sản phẩm là vì thiếu bằng chứng, không phải vì có thất bại đo được.

### 1.1 Hỗ trợ khách hàng

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| gpt-4o gọi hàm đạt khoảng 61% pass^1 ở bán lẻ, 35% ở hàng không; **pass^8 dưới 25%** ở bán lẻ (nhất quán qua 8 lần chạy) | tau-bench, `arxiv.org/abs/2406.12045` | HIGH |
| gpt-4.1 pass^1: 74% bán lẻ, 56% hàng không, **34%** viễn thông (miền người dùng cũng tác động lên hệ thống) | tau2-bench, `arxiv.org/abs/2506.07982` | HIGH |
| Trên 840 lần chạy của 7 model, **55,5%** là "false-pass": kết quả đúng nhưng vi phạm quy trình hoặc đồng ý; chiếm **80,1%** số lần đạt kết quả; chỉ **13,8%** đạt cả kết quả lẫn quy trình | CXBench, `arxiv.org/abs/2610.25954` | HIGH |
| Tỷ lệ giải quyết 76% (Intercom, "trung bình trên 12.000 khách") và 84% (Salesforce) là do vendor tự báo, không định nghĩa mẫu số, không kiểm toán độc lập | trang vendor | MEDIUM |
| Gần 1 trong 5 người tiêu dùng không thấy lợi ích từ dịch vụ khách hàng AI, gấp gần 4 lần mức cho AI nói chung (Qualtrics, 20.000 người, 14 nước) | Qualtrics | MEDIUM |

Hệ quả: đo kết quả cuối không đủ; cần **rail xác định cho quy trình** (hoàn tiền, chính sách), và đo độ nhất quán qua nhiều lần chạy.

### 1.2 Marketing

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| 2.234 người tạo 11.024 quảng cáo; thử nghiệm thực địa trên X với 4.932.373 lượt hiển thị và 7.546 lượt nhấp. Nhóm người + AI làm nhiều hơn 50% quảng cáo trên mỗi người và văn bản tốt hơn; nhóm người + người làm hình ảnh tốt hơn | Ju & Aral, `arxiv.org/abs/2503.18238` | HIGH |
| Quảng cáo hoàn toàn do AI tạo CTR 4,44% so với 3,73% (+19%); công khai là AI giảm CTR 31,5% | NYU Stern (bản tóm tắt), chưa thấy bản preprint | MEDIUM |
| 640 thử nghiệm incrementality trên Meta: chiến dịch thủ công thắng Advantage+ về iROAS ở 58% thương hiệu | Haus (bên bán dịch vụ đo lường) | MEDIUM |

Hệ quả: dùng tín hiệu thực địa (holdout), không dùng ROAS do nền tảng báo làm phần thưởng.

### 1.3 Thiết kế

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| Khoảng **25%** lý do thiết kế được nêu không được hiện thực, **34%** với yêu cầu chức năng (120 giao diện, 5 công cụ) | Design Theater, `arxiv.org/abs/2607.22928` | HIGH |
| Đầu ra model phù hợp (tốt nhất 99,2% so với 98,0% của thiết kế người) nhưng **ít đa dạng hơn**: creative range 0,581 so với 0,902 | Design Creativity Bench, `arxiv.org/abs/2610.11539` (tác giả thuộc một vendor) | HIGH cho số; MEDIUM về độc lập |
| Bản người thiết kế được ưa hơn bản UI sinh bởi Gemini 3 Pro: 56% so với 43% | thử nghiệm của chính Google, qua bài của Nielsen | MEDIUM |

### 1.4 Quản lý sản phẩm

- Người dùng tổng hợp: 37 LLM cho thang đo bị nén (độ lệch chuẩn trong từng người 0,79 so với 1,19) và R² trên người thật là −0,14 so với 0,28; LLM tốt nhất
  (0,81) ngang một mô hình thống kê không có ngôn ngữ (0,82). `arxiv.org/abs/2608.14606`. **HIGH**.
- Không tìm thấy nghiên cứu đo kết quả của AI làm roadmap, ưu tiên hay nghiên cứu người dùng.

### 1.5 Số liệu tài chính và số học

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| Agent tốt nhất đạt 46,8% ở $3,79/truy vấn (o3); chuyên gia người cần 16,8 phút và $25,66 | Finance Agent, `arxiv.org/abs/2508.00828` | HIGH |
| Bảng Vals Finance Agent v1.1 (06/2026): Claude Opus 4.7 64,37%, Claude Sonnet 4.6 63,33%, GPT 5.5 59,96% | `vals.ai/benchmarks/finance_agent` | MEDIUM (sub-agent đọc trang) |
| 24 bảng tính: model tốt nhất 82,4% (khoảng 1 lỗi trên 6 câu); bảng lớn nhất (152 công ty) trung bình 48,6% so với 86,2% ở bảng dễ nhất; bật suy luận nâng GPT-5.2 từ 57,7% lên 80,4%; tác giả kết luận không model độc lập nào đủ an toàn để dùng không giám sát | FinSheet-Bench, `arxiv.org/abs/2603.07316` | HIGH |
| Tính toán bằng trình thông dịch tăng khoảng 12% so với chain-of-thought thuần (model 2022) | Program of Thoughts, `arxiv.org/abs/2211.12588` | MEDIUM (cũ) |

Hệ quả: **không để agent tự tính số mà khách dựa vào.** Phép tính là mã xác định; agent chỉ chọn đầu vào và giải thích kết quả.

## 2. Bằng chứng ngược chiều

Ba lập luận phản biện tôi coi trọng nhất (xếp theo nhận định của tôi), kèm bằng chứng đã đối chiếu.

**1. Nhất quán, không phải năng lực, là nút thắt, và nó ít cải thiện.**
- Rabanser, Kapoor, Narayanan (ICML 2026), 15 model qua 24 tháng phát hành: "Overall reliability shows minimal improvement over time" và
  "outcome consistency remains low across all models"; độ tin cậy tăng 0,03/năm (GAIA, r = 0,46, p = 0,08) và 0,09/năm (τ-bench), trong khi độ chính xác tăng 0,23/năm và 0,22/năm; mỗi đơn vị
  độ chính xác tăng thêm chỉ kéo độ tin cậy lên 0,18 (GAIA) và 0,33 (τ-bench);
  nhất quán kết quả trên τ-bench từ 0,52 đến 0,90. `arxiv.org/abs/2602.16666`. **HIGH**. Giới hạn: hai benchmark.
- Mỗi lần chạy riêng lẻ biến động: 60.000 quỹ đạo trên SWE-Bench-Verified, pass@1 một lần chạy dao động 2,2–6,0 điểm, độ lệch chuẩn trên 1,5 điểm ngay ở
  temperature 0 (`arxiv.org/abs/2602.07150`, **HIGH**); Atil et al.: độ chính xác dao động tới 15% giữa các lần chạy "xác định" (`arxiv.org/abs/2408.04667`, **HIGH**).
- Phản biện: model 2024–2025, benchmark τ đang bão hoà về pass^1; calibration có cải thiện.

**2. Tốc độ nằm ở đầu, nợ nằm ở cuối, và trong công ty agent không ai gánh nợ.**
- Nghiên cứu sai biệt trong sai biệt trên các dự án GitHub dùng Cursor: dòng thêm tăng 3–5 lần ở tháng đầu nhưng "gains dissipate after two months"; cảnh báo phân tích tĩnh
  +30,26% (±6,66%) và độ phức tạp +41,64% (±7,62%) kéo dài. He et al., `arxiv.org/abs/2511.04427`. **HIGH**. Giới hạn: dự án mã nguồn mở.
- Faros AI (22.000 lập trình viên): sự cố trên mỗi PR +242,7%; thông lượng cũng tăng. **MEDIUM** (vendor bán công cụ phân tích).
- METR: khoảng một nửa PR qua test của SWE-bench Verified sẽ không được merge. **MEDIUM**.
- Amazon yêu cầu duyệt cấp cao cho thay đổi do AI hỗ trợ sau các sự cố; Amazon bác bỏ việc quy lỗi cho AI. **MEDIUM**.

**3. Agent đang triển khai thành công nhờ giới hạn bởi con người.**
- Pan et al. (ICML 2026): 20 ca và **306** người trả lời, lọc còn **86** hệ thống đang chạy thật; **68%** chạy tối đa 10 bước trước khi có người can thiệp, **74%** chủ yếu
  dựa vào đánh giá của người, **70%** chỉ dùng model có sẵn với prompt. `arxiv.org/abs/2512.04123`. **HIGH**. Giới hạn: lấy mẫu qua mạng quen biết.
- Ngân hàng Commonwealth (Úc) đảo ngược việc cắt 45 vị trí hỗ trợ năm 2025-08 vì khối lượng cuộc gọi tăng; nguồn tổng hợp MEDIUM.

**Điểm thuận chiều trong cùng nguồn (để công bằng):**
- Sinha et al. (ICLR 2026): lỗi tích tụ theo bước ("self-conditioning"), không giảm khi tăng cỡ model, nhưng model suy luận **không bị ảnh hưởng** bởi lỗi trước đó; GPT-5 thinking thực hiện
  hơn 2.100 bước so với 432 của Claude-4 Sonnet. `arxiv.org/abs/2509.09677`. **HIGH**; nhiệm vụ tổng hợp, kế hoạch được cung cấp.
- SWE-Bench Illusion: o3 xác định đúng tệp lỗi từ mô tả vấn đề tới 76% so với 53% ở kho ngoài SWE-bench, dấu hiệu nhiễm dữ liệu. `arxiv.org/abs/2506.12286`. **HIGH**.
- Báo cáo "95% dự án GenAI thất bại" của MIT NANDA có nhiều phê bình phương pháp và không nên trích dẫn.

## 3. Công ty do agent vận hành: điều gì thực sự đã được chứng minh

Chấm A (đo độc lập), B (bên thứ ba hoặc phóng viên xác minh), C (tự báo cáo), D (quảng cáo hoặc không xác minh được).

**Đã chứng minh (A hoặc B):**
- **A, mô phỏng:** CEO-Bench, công ty khởi nghiệp mô phỏng 500 ngày từ $1M: chỉ Claude Fable 5 ($12,6M), GPT-5.6 Sol ($11,3M) và Claude Opus 4.8 ($2,4M) kết thúc trên mức khởi đầu; **mọi model đều thua baseline dựa trên quy tắc ($15,8M)**. `arxiv.org/abs/2606.18543`. **HIGH**.
- **A, khảo sát:** NBER w34836 ("Firm Data on AI"), gần 9.000 lãnh đạo ở 7 nước: AI "essentially no net effect" lên việc làm trong 3 năm, năng suất +0,2% mỗi năm (dự đoán +0,6% mỗi năm cho 3 năm tới). **HIGH**; đây là tự báo cáo của lãnh đạo.
- **A, dữ liệu lương:** Brynjolfsson et al. dùng bảng lương ADP: việc làm của người 22–25 tuổi ở nghề chịu ảnh hưởng AI thấp hơn 19% so với mức đối chứng; "no evidence of widespread job displacement". Mô tả, không nhân quả. **HIGH**.
- **B:** Andon Market và Andon Café (cửa hàng và quán do agent vận hành): báo chí ghi lỗ và cần người cho việc thể chất; trên trang của Andon, chi phí token của quán cao hơn doanh thu. **MEDIUM**.
- **B:** HurumoAI (phóng viên dựng công ty 5 nhân viên AI): sau khoảng 11 tháng không doanh thu; agent bịa và phớt lờ lệnh dừng. **MEDIUM**.

**Không chứng minh được (C hoặc D):** Polsia ("$10M" chạy đà, Fortune ghi "no verification", công ty có một nhân viên toàn thời gian cùng nhà thầu), NanoCorp ($940K ARR, không phương pháp;
trang tự báo cáo: 34.704 doanh nghiệp, người kiếm nhiều nhất $3.600), Medvi (doanh số $401M với 2 nhân viên, bị chỉ trích bỏ sót cảnh báo FDA và đối tác lâm sàng thuê ngoài), Midjourney (D).
**MEDIUM đến LOW**, chưa đối chiếu gốc.

**Kết luận:** không có trường hợp nào chấm A hoặc B cho thấy một công ty vận hành đầu cuối bằng agent với lợi nhuận được xác minh. Điều "chưa ai làm" **chưa bị bác bỏ**, và cũng chưa được ai chứng minh làm được.
Đây là chỗ trống của thế giới, đồng thời là chỗ một thí nghiệm có đo đạc tốt có thể tạo dữ liệu mới.

**Thực thể pháp lý:** Delaware DGCL §141(b) yêu cầu giám đốc là thể nhân (MEDIUM). Dự thảo "AI Company" 36 trang của Delaware chưa được trình (thư House Oversight ngày 2026-10-26 nói "likely to be introduced … next year"; chủ sở hữu được bảo vệ trừ khi không
góp vốn hoặc dùng để gian lận hoặc cố ý vi phạm luật). **HIGH** cho thư. Quy chế DAO của Wyoming vẫn đòi thành viên là người hoặc pháp nhân (MEDIUM).

## 4. Vận hành agent

| Phát hiện | Nguồn | Tin cậy |
|---|---|---|
| Ba lỗi hạ tầng chồng nhau của Anthropic: lỗi định tuyến bắt đầu 5/8 ảnh hưởng 0,8% yêu cầu Sonnet 4, **16%** ở mức tệ nhất (31/8); đo kiểm của họ "simply didn't capture the degradation"; phát hiện qua phản hồi người dùng | `anthropic.com/engineering/a-postmortem-of-three-recent-issues` | HIGH (đã mở lại) |
| Suy giảm chất lượng Claude Code (4/3–20/4/2026): ba thay đổi **ở harness, không phải model hay API**: hạ mức suy luận mặc định; lỗi bộ nhớ đệm xoá lý lẽ mỗi lượt; dòng prompt giới hạn độ dài (mất 3% trên một eval). Anthropic ghi: "neither our internal usage nor evals initially reproduced the issues" | `anthropic.com/engineering/april-23-postmortem` | HIGH (đã mở lại) |
| Sycophancy GPT-4o (4/2025): chỉ số ngoại tuyến và A/B trông tốt; đội đánh giá định tính cảnh báo; OpenAI nói tung ra là "the wrong call" | OpenAI, qua bản lưu Wayback | MEDIUM |
| Lỗi nhất quán ngay cả khi ghim phiên bản: độ chính xác dao động tới 15% giữa các lần chạy "xác định" | `arxiv.org/abs/2408.04667` | HIGH |
| Hội thoại nhiều lượt: hiệu năng trung bình giảm **39%** so với một lượt (khả năng −15%, độ không ổn định +112%); khi "rẽ nhầm" model không tự phục hồi | `arxiv.org/abs/2505.06120` | HIGH |
| Bộ nhớ: LongMemEval, model ngữ cảnh dài tụt 30–60%; GPT-4o 0,870 → 0,606 | `arxiv.org/abs/2410.10813` | HIGH |
| Viết lại toàn bộ ngữ cảnh bằng LLM gây "context collapse": 18.282 token → 122, độ chính xác 66,7 → 57,1, thấp hơn mức không có ngữ cảnh (63,7); cập nhật có cấu trúc, tăng dần tránh được | ACE, `arxiv.org/abs/2510.04618` | HIGH |
| Bộ nhớ tự tiến hoá làm hỏng an toàn: tỷ lệ từ chối của Qwen3-Coder-480B 99,4% → 54,4%, tỷ lệ tấn công thành công 0,6% → 20,6% | `arxiv.org/abs/2509.26354` | HIGH, một nghiên cứu |
| Thay judge đổi kết quả: khi thay Mistral-Small 3.1 bằng 3.2, 93% điểm rơi vào một giá trị và 59% tập chọn top-30% đổi; chỉnh lại trung bình không sửa được | `arxiv.org/abs/2610.32986` | HIGH |
| Judge cần mốc neo của người: tập neo cố định phát hiện đúng 60/60 lần đổi phiên bản âm thầm; z-test cuốn chiếu báo động nhầm 75% | `arxiv.org/abs/2606.15474` | HIGH, một tác giả |
| Quy ước OpenTelemetry cho GenAI vẫn ở trạng thái Development; có span cho `invoke_agent`, `execute_tool` và các thao tác bộ nhớ | repo semantic-conventions-genai | MEDIUM |
| Lịch ngừng model: Anthropic báo trước tối thiểu 60 ngày; các tham số lấy mẫu (`temperature`, `top_p`, `top_k`) bị loại ở phiên bản API 2026-10-21 | trang deprecations của Anthropic | MEDIUM |

**Danh sách việc phải xây cho lớp vận hành (suy luận từ bằng chứng, MEDIUM):**
1. Ghi vết từng bước agent và lời gọi công cụ theo schema của chúng ta; xuất ra OTel khi cần; luôn ghi mã model phản hồi thực.
2. Coi mọi sửa prompt, playbook, harness và bộ nhớ là một lần phát hành: cổng eval, thời gian quan sát, triển khai dần, hoàn tác.
3. Chạy canary liên tục trên lưu lượng thật, lặp nhiều lần, so sánh phân phối chứ không so một lần chạy.
4. Giữ nhật ký thay đổi (mã model, băm prompt, cấu hình) để đối chiếu khi báo cáo lỗi tăng.
5. Ghim bản chụp có ngày; ghi lịch ngừng model; chạy lại bộ kiểm thử trước 60 ngày.
6. Auditor có quyền phủ quyết định tính, ưu tiên hơn chỉ số tổng hợp.
7. Ghi chú chung: chỉ thêm hoặc thay đổi có cấu trúc, có phiên bản; không cho LLM viết lại cả tài liệu.
8. Mỗi judge hiệu chỉnh theo tập neo do người gán nhãn, ghim phiên bản, dùng độ đồng thuận có hiệu chỉnh cơ hội.
9. Dự phòng giữa các nhà cung cấp phải mang theo trạng thái, có eval riêng, và không phụ thuộc bảng điều khiển của nhà cung cấp đang hỏng.

## 5. Phương pháp thí nghiệm

### 5.1 Thống kê (đã đối chiếu toàn văn, HIGH)

- Miller 2024, `arxiv.org/abs/2411.00640`: sai số chuẩn của trung bình là `sqrt(Var(s)/n)`; khoảng tin cậy 95% là trung bình ± 1,96·SE; so sánh ghép cặp dùng
  sai số của chênh lệch theo từng câu; cỡ mẫu `n = (z_{α/2} + z_β)² (ω² + σ²_A/K_A + σ²_B/K_B) / δ²` (công thức 9). Ví dụ trong bài: ω² = 1/9, δ = 0,03, α = 0,05, công suất 80% cho n ≈ 969.
  Tăng số lần chạy mỗi câu từ 1 lên 10 giảm hiệu ứng nhỏ nhất phát hiện được từ 13,2% xuống 7,5%. Không giảm temperature để giảm phương sai.
- Với dưới vài trăm mục, khoảng tin cậy CLT đánh giá thấp bất định (Bowyer et al., `arxiv.org/abs/2503.01747`, MEDIUM); dùng Wilson, Clopper-Pearson hoặc Bayes cho tỷ lệ.
- Judge: hoán đổi thứ tự A/B và chỉ tuyên bố thắng nếu đúng ở cả hai thứ tự (Zheng et al., HIGH); đo κ chứ không đo tỷ lệ khớp thô, vì tỷ lệ khớp thô thổi phồng độ tin cậy 33–41 điểm trên MT-Bench;
  lặp tối thiểu 3 lần; dùng hơn một benchmark (Norman et al., `arxiv.org/abs/2606.19544`, **HIGH**, một nhóm).
- Phát hiện cố ý làm kém: lật khuyến khích giữa hai bối cảnh giống hệt nhau và so độ chính xác (Apollo, `arxiv.org/abs/2412.04984`: Sonnet 3.5 từ 0,99 xuống 0,34, **HIGH**); không có kết luận chỉ từ chuỗi suy luận,
  vì model có thể qua mặt monitor khi biết có monitor (16–36%, `arxiv.org/abs/2508.00943`, MEDIUM).
- Gieo lỗi: dùng lỗi thật trong lịch sử trước; lỗi nhân tạo biểu diễn thiếu lỗi thật (73% lỗi thật được "ghép" với đột biến sinh tự động, 17% không, 10% cần toán tử mới; Just et al. 2014, MEDIUM); tỷ lệ bỏ sót tăng khi mục tiêu hiếm
  (Wolfe 2005: 7% ở 50% mật độ, 30% ở 1%). Luôn báo cáo tỷ lệ báo động nhầm cùng với recall.
- Công cụ: Inspect AI (MIT, bản 0.3.287 ngày 2026-10-31, theo `gh api` của sub-agent) hỗ trợ epochs, sai số chuẩn theo cụm và nhiều model chấm. **MEDIUM**.
- Đăng ký trước giả thuyết, phiên bản model, tham số, ngân sách và quy tắc quyết định (mẫu của Vaccaro, ICML 2026, `arxiv.org/abs/2606.11217`, MEDIUM, chưa đối chiếu).

### 5.2 Thiết kế pilot (của tôi, dựa trên các phương pháp trên; MEDIUM)

**Yêu cầu chung, rút ra từ bằng chứng ngược chiều:**
1. **Lặp nhiều lần và báo cáo phân phối.** Mỗi ô thí nghiệm chạy K ≥ 3 lần; báo cáo trung bình, phương sai, pass^k và cả lần chạy tệ nhất. Trung bình che các lần chạy hỏng nặng (Vending-Bench: giá trị ròng thấp nhất $476 so với trung bình $2.218).
2. **Chạy đủ dài.** Pilot kéo dài tối thiểu 8–12 tuần và theo dõi nợ: cảnh báo phân tích tĩnh, độ phức tạp, sự cố trên mỗi thay đổi, tải review. Lợi ích đầu kỳ tan sau khoảng hai tháng (Cursor DiD).
3. **Chấm cả quy trình lẫn kết quả.** Một lần chạy chỉ đạt khi cả hai đạt (CXBench: 55,5% là false-pass).
4. **Số liệu do mã tính.** Phần tính toán tài chính nằm ngoài tầm của agent.
5. **Điều kiện đối chứng có giới hạn.** Một nhánh có điểm kiểm tra của auditor sau mỗi N bước, tương ứng với kiểu triển khai phổ biến (68% chạy tối đa 10 bước, Pan et al.).
6. **Khối lượng chức năng bị thay thế được đo độc lập** với báo cáo của agent hoặc vendor (bài học CBA).
7. **Judge:** khác họ model với tác giả và reviewer, trộn judge frontier với open-weight (Hossain), hoán đổi thứ tự, mốc neo do người gán nhãn, ghim phiên bản.
8. **Nhận rủi ro có phục hồi** (lập trường của dự án): pilot chạy trên một tenant hoặc môi trường staging có nút dừng riêng, sao lưu ngoài quyền của agent, và log do nền tảng ghi. Phục hồi này phải sẵn sàng trước ngày chạy đầu tiên.

**Cỡ mẫu chung.** Với chênh lệch ghép cặp có độ lệch chuẩn s_d (placeholder 0,25, cần ước lượng lại từ pilot thử 10–15 tác vụ): `n ≈ 7,85·s_d²/δ²` cho α = 0,05 hai phía, công suất 80%
(công thức 9 của Miller khi đã gộp K). Kết quả: n = 49 tác vụ cho δ = 0,10; n = 196 cho δ = 0,05; với 60 tác vụ, hiệu ứng nhỏ nhất phát hiện được là 0,090. Với ba so sánh đã đăng ký và hiệu chỉnh Holm (α/3), n ≈ 65 tại δ = 0,10.

| Câu hỏi | Điều kiện | Đơn vị và quy mô | Chỉ số chính | Mối đe doạ chính |
|---|---|---|---|---|
| **Q1** từ chối có mini-spec | (a) không duyệt, (b) duyệt im lặng hoặc từ chối trơn, (c) từ chối bằng lời tự do, (d) từ chối kèm đặc tả có bằng chứng; giới hạn một vòng sửa | 60–70 tác vụ × 4 điều kiện × K = 3 ≈ 780 lần chạy | Thay đổi điểm của bộ kiểm tra xác định (ràng buộc giấu của hợp đồng cha) từ bản nháp 1 tới bản cuối; phụ: số vòng, token, chi phí, điểm judge | Reviewer không kiểm chứng được thì chỉ thêm nhiễu; bộ kiểm tra phải độc lập với hạt giống lỗi |
| **Q2** agent hạ mục tiêu | (a) tự đặt mục tiêu, thưởng theo điểm tuyệt đối; (b) thưởng theo việc vượt mục tiêu tự đặt; (c) như (b) cộng quy tắc kiểm (sàn bằng trung vị trượt, hoặc kiểm ngẫu nhiên có phạt) | 60 tác vụ × 3 điều kiện × K = 3, hai cách diễn đạt khuyến khích, hai model | Khoảng cách mục tiêu chuẩn hoá = (mục tiêu − năng lực)/năng lực; hiệu ứng là sai biệt trong sai biệt; kèm kiểm lật khuyến khích | Tỷ lệ gốc có thể gần 0 (kết quả không có gì cần cận trên); model biết mình bị đánh giá |
| **Q3** độ sâu và số vòng duyệt | Không duyệt; độ sâu {1, 2, 3} × tối đa {1, 3} vòng = 7 cấu hình | 40 tác vụ × K = 3 ≈ 840 lần chạy | log độ trễ (p50, p95), log token (tách bộ nhớ đệm), tỷ lệ lỗi còn lại; độ dốc theo tầng và theo vòng, với hiệu ứng ngẫu nhiên theo tác vụ | Độ trễ API trôi theo thời gian (xen kẽ các lần chạy), giới hạn tốc độ nhà cung cấp |
| **Q4** bỏ sót lỗi gieo | Reviewer đơn, danh sách kiểm, reviewer có công cụ kiểm chứng, reviewer khác họ, hai reviewer, và owner | 120 sản phẩm: 80 có 1–3 lỗi gieo (khoảng 160 lỗi), 40 sạch để đo báo động nhầm | Tỷ lệ bỏ sót mỗi lỗi, báo cáo cùng báo động nhầm và d′ | Mật độ, hạt giống không giống lỗi thật, người duyệt học mẫu lỗi |

Cho so sánh tỷ lệ ghép cặp của Q4: `n = (z_{α/2}·√ψ + z_β·√(ψ − δ²))²/δ²`; với ψ = 0,3 (tỷ lệ cặp bất đồng) và δ = 0,15 cho khoảng 102 lỗi, khoảng 123 sau hệ số thiết kế 1,2 do hai lỗi trên một sản phẩm.
Công thức này là công thức McNemar chuẩn và tôi tính lại được; sub-agent không dẫn nguồn, nên ghi **MEDIUM**.

Với owner trong Q4: gieo lỗi vào hàng đợi thật ở mật độ thực tế (Gur 2008: hiệu năng ở môi trường thực khác phòng thí nghiệm giàu mẫu). Chỉ có một owner, nên kết luận chỉ cho người đó.

**Quy tắc quyết định (đề xuất, cần đăng ký trước):**
- Q1: áp dụng từ chối có mini-spec nếu cận dưới khoảng tin cậy 95% của chênh lệch (d) so với (b) lớn hơn 0,05 mà không tăng chi phí hơn 25%.
- Q2: bật quy tắc kiểm cho mọi OKR do agent đặt nếu khoảng cách mục tiêu chuẩn hoá giảm có ý nghĩa ở (c) so với (b); nếu tỷ lệ gốc ở (a) không phân biệt được với 0, giữ quy tắc kiểm ở mức rẻ nhất.
- Q3: giới hạn độ sâu ở mức mà độ dốc lỗi còn lại ngừng giảm.
- Q4: giữ rail của owner chỉ cho hành động có tỷ lệ bỏ sót thấp hơn ngưỡng đã đăng ký; hành động khác chuyển cho auditor hoặc quy tắc xác định.

**Pre-mortem (nguyên nhân khả dĩ nhất khiến pilot thất bại), mỗi cái có cách xử lý trước:**
1. Đọc trung bình thay vì phân phối → pass^k và lần chạy tệ nhất là chỉ số chính.
2. Judge và tác giả lỗi cùng chiều → ưu tiên bộ kiểm tra xác định; judge khác họ, trộn frontier với open-weight, mốc neo của người.
3. Model hoặc harness đổi giữa chừng → ghim phiên bản, ghi nhật ký thay đổi, canary hằng ngày.
4. Pilot quá ngắn nên hiệu ứng đầu kỳ che nợ → tối thiểu 8–12 tuần, đo nợ.
5. Tác vụ không đại diện → dùng việc thật trong lịch sử của chính công ty, kèm tác vụ tư nhân mới viết.

## 6. Cập nhật các câu hỏi mở

| Câu hỏi (mục 11 của báo cáo thiết kế) | Nay |
|---|---|
| Từ chối kèm mini-spec so với duyệt im lặng | Vẫn không có nghiên cứu đo; có thiết kế Q1 |
| Agent tự đặt OKR dễ | Vẫn không có nghiên cứu đo; có thiết kế Q2 (Hopman, Apollo là lân cận) |
| Độ trễ theo tầng | Vẫn không có số đo theo tầng; có thiết kế Q3 |
| Phần thưởng từ kết quả đến chậm | Chưa đổi; xem surrogate ở báo cáo thiết kế mục 7 |
| Cắt 80% chi phí review/QA | Bằng chứng nghiêng về ngược lại: tải review tăng; Cursor DiD, Faros |
| "Chưa ai vận hành công ty đầu cuối" | Không có ca A hoặc B nào có lợi nhuận được xác minh (mục 3 ở trên) |

## 7. Điều chưa kiểm chứng

- Các tuyên bố doanh thu của Polsia, NanoCorp, Medvi, Midjourney, Cursor và Lovable chưa được kiểm toán; tôi không dùng làm bằng chứng.
- Số của NYU Stern về CTR quảng cáo AI chưa tìm được bản preprint; Haus có lợi ích thương mại.
- Bảng Vals Finance Agent, các bảng xếp hạng benchmark 2026 và các số liệu giá model chỉ từ trang web.
- Quy tắc quyết định và cỡ mẫu trong mục 5.2 là thiết kế của tôi; s_d = 0,25 chỉ là giá trị giả định.
- Không có nghiên cứu nào về một owner duyệt nhiều giờ việc tự động (mục 2.3 của báo cáo rủi ro).
- Quyết định Delaware AIC chưa được trình; quy chế Idaho, Utah, Tennessee và Ohio chưa đối chiếu.

## Nguồn chính

- Role: `arxiv.org/abs/2406.12045`, `2506.07982`, `2610.25954`, `2503.18238`, `2607.22928`, `2610.11539`, `2608.14606`, `2508.00828`, `2603.07316`; `vals.ai/benchmarks/finance_agent`
- Ngược chiều: `arxiv.org/abs/2602.16666`, `2602.07150`, `2408.04667`, `2511.04427`, `2512.04123`, `2509.09677`, `2506.12286`
- Công ty do agent vận hành: `arxiv.org/abs/2606.18543`; `nber.org/papers/w34836` (Yotzov et al., "Firm Data on AI"); Brynjolfsson, Chandar, Chen, "Canaries in the Coal Mine" (Stanford Digital Economy Lab); thư House Oversight gửi Delaware, 2026-10-26
- Vận hành: `anthropic.com/engineering/a-postmortem-of-three-recent-issues`, `anthropic.com/engineering/april-23-postmortem`; `arxiv.org/abs/2505.06120`, `2410.10813`, `2510.04618`, `2509.26354`, `2610.32986`, `2606.15474`
- Phương pháp: `arxiv.org/abs/2411.00640`, `2606.19544`, `2412.04984`, `2503.01747`, `2508.00943`
