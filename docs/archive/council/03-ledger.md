# Sổ bằng chứng và rủi ro (Evidence and Risk Ledger), v2

> Bản nháp số 03 của hội đồng, sửa sau sáu phản biện (`challenge-01` … `challenge-06`; nhật ký ở [`03-response.md`](03-response.md),
> bản trước ở [`03-ledger-v1.md`](03-ledger-v1.md)). Ngày: 2026-11-01.
> Đầu vào: [`research-ai-autonomous-system-evidence.md`](../research-ai-autonomous-system-evidence.md) (EV),
> [`…-design.md`](../research-ai-autonomous-system-design.md) (DS), [`…-risk.md`](../research-ai-autonomous-system-risk.md) (RK),
> [`…-pilot.md`](../research-ai-autonomous-system-pilot.md) (PL), [`brainstorm-ai-autonomous-system.md`](../brainstorm-ai-autonomous-system.md) (BS),
> [`papers/INDEX.md`](../papers/INDEX.md), ADR trong `docs/adr/`. Mục 4–5 **không phải tư vấn pháp lý**.

## 0. Cách đọc

**Quyết định của owner.** Toàn bộ hệ thống chạy thật cùng một lúc và được sửa trong khi chạy. Không thí nghiệm. Không bí mật.
Không giới hạn sáng kiến của agent (không trần chi phí, trần tự chủ, giới hạn phạm vi, cổng duyệt, giai đoạn, tự động dừng theo
hiệu năng). Điểm cố định: **F1** nút dừng thủ công của owner; **F2** credential không bao giờ ở trong ngữ cảnh agent; **F3** tuân
thủ pháp luật.

**Phán quyết mặc định của Chair (chờ owner xác nhận; ghi ESCALATE trong `03-response.md`).**
- **R1 Rail.** Các rail của `AGENTS.md` giữ nguyên: xác thực, pause và stop, merge, cộng trường credential chỉ ghi. Gọi rail merge
  là **F4**: merge do owner kéo. Bỏ F4 thì agent sửa được chính mã của nút dừng (ADR-0012: Remit tự sửa qua chain của nó).
- **R2 "Mở".** Mở với owner và người của công ty. Phương pháp được công bố sau khi một người có tên kiểm pháp lý. Ngoại lệ thứ hai
  bên cạnh credential: **tập đánh giá niêm** (ngưỡng phát hiện, vị trí canary, tập neo giữ riêng, nhãn lỗi gieo) không vào ngữ cảnh
  agent bị đo và không công bố. Sổ này chỉ dùng định nghĩa đó.
- **R3 Thí nghiệm.** Lặp một tác vụ để đo độ nhất quán, phát lại dữ liệu đã ghi, gieo lỗi vào việc đã ghi là **quan sát** (được
  phép). Nhánh gán ngẫu nhiên, holdout, A/B trên khách là **thí nghiệm** (không làm).
- **R4 Luật thắng.** Khi F3 va với "không giới hạn", F3 thắng. Mục 4 liệt kê điều luật buộc quay lại.
- **R5 Lớp hiệu ứng.** Mỗi rủi ro ghi hiệu ứng là đảo ngược được, bù trừ được, hay không đảo ngược (mục 2).
- **R7 Khoảng hở của hiện thực tham chiếu Remit** được ghi như rủi ro riêng (R18).

**Nhãn.** HIGH: nguồn gốc đã đọc toàn văn (bởi tôi, hoặc bởi thành viên hội đồng có ghi tên phản biện). MEDIUM: sub-agent, nguồn thứ
cấp, vendor. LOW: snippet hoặc suy luận. **RECHECKED**: tôi tự tra con số trong văn bản trích từ PDF ở `docs/papers/` (pypdf, venv tạm
ở `/tmp`, đã xoá). Không có chữ RECHECKED là tôi chưa tra. Nội dung không có bằng chứng ghi **Đề xuất của hội đồng (giả thuyết)**.

**Phạm vi kiểm lại.** v1: hơn 70 nhóm số trong 66 tài liệu. v2: thêm 34 nhóm số chưa tra trước đó (trong 34 tài liệu), cộng tra lại
các số phản biện chỉ ra. Một con số mang RECHECKED ở v1 là **sai** (C11: tôi ghép nhầm độ dốc); đã sửa. Không kiểm được: nguồn không phải
PDF (blog vendor, SEC, văn bản luật, điều khoản vendor), MAST v2 (kho có v3).

**Giả định:**
- Assumption: "blueprint" claims are the load-bearing claims of the four research reports, the brainstorm and the four council drafts as the challenges quote them, because the challenges read all four drafts and I revised against their quotations.
- Assumption: the first customers are businesses (merchants using the pilot application); sole traders among them are natural persons, so consumer and personal-data rules still reach part of the base.
- Assumption: the company is a Vietnamese legal entity serving customers in the US and the EU, because challenge-04 reads the law on that basis and Vietnamese duties apply where the entity sits.
- Assumption: the owner's manual stop (F1) is deterministic code outside any agent's reach, because a stop an agent can disable is not a stop (AGENTS.md, rails; Palisade, C48).
- Assumption: the Chair rulings R1–R7 hold as defaults until the owner confirms or overrides them, because the coordinator issued them as defaults pending the owner.

## 1. Sổ khẳng định (claim register)

Cột "Bác bỏ nếu": kết quả nào làm khẳng định sai.

### 1.1 Năng lực của agent theo role

| ID | Khẳng định | Số chính xác | Nguồn | Nhãn | Bác bỏ nếu | Kiểm lại |
|---|---|---|---|---|---|---|
| C01 | Agent tốt nhất tự hoàn thành khoảng 30% tác vụ của công ty mô phỏng; hành chính, tài chính, data science thấp nhất | Gemini-2.5-Pro 30,3% (điểm 39,3%, $4,2/tác vụ); Claude-3.7-Sonnet 26,3% ($4,1) | 2412.14161; DS §8 | HIGH | Agent mới đạt đa số tác vụ tài chính, hành chính | RECHECKED |
| C02 | Việc freelance thật: tự động hoá rất ít | 2,5% trên 240 dự án; tổng $143.991; tốt nhất kiếm $1.720 | 2510.26787 | HIGH; 16,1% sau 8 tháng chỉ MEDIUM | Đo độc lập mới cao gấp nhiều lần | RECHECKED |
| C03 | Tác vụ kinh tế: model tốt nhất gần chuyên gia | Claude Opus 4.1 thắng hoặc hoà 47,6% (tập gold) | 2510.04374 | HIGH | Người chấm độc lập cho thấp hơn rõ | RECHECKED |
| C04 | Phân tích tài chính bằng agent chưa đủ tin cậy | o3 46,8%, $3,79/truy vấn (chỗ khác ghi $3,78, 3,1 phút); chuyên gia 16,8 phút, $25,66 | 2508.00828 | HIGH | Tập này >90% | RECHECKED |
| C05 | Bảng tính tài chính: không model nào đủ an toàn để dùng không giám sát (tác giả) | 82,4% (khoảng 1 lỗi/6 câu); bảng lớn nhất 48,6% so với 86,2% | 2603.07316 | HIGH | ≥99% trên bảng lớn | RECHECKED |
| C06 | Điều hành công ty mô phỏng 500 ngày: mọi model thua baseline quy tắc | Baseline $15,8M; tốt nhất $12,6M; khởi đầu $1M | 2606.18543 | HIGH; mô phỏng | Vượt baseline trên nhiều seed | RECHECKED |
| C07 | Hỗ trợ khách: đúng kết quả sai quy trình phổ biến | 55,5% của **mọi** 840 lần chạy là false-pass = 80,1% **của lần đạt kết quả**; 13,8% đạt cả hai | 2610.25954 | HIGH | Tỷ lệ false-pass thấp ở model mới | RECHECKED |
| C08 | Nhất quán qua nhiều lần chạy thấp | gpt-4o pass^1 khoảng 61% bán lẻ, 35% hàng không, pass^8 khoảng 25%; gpt-4.1 34% viễn thông | 2406.12045, 2506.07982 | HIGH | pass^8 gần pass^1 | RECHECKED |
| C09 | Công cụ UI sinh không hiện thực điều nó tuyên bố; đầu ra ít đa dạng | 25% lý do thiết kế, 34% yêu cầu chức năng (120 giao diện). Creative range 0,581 so với 0,902 của người; phù hợp tốt nhất 99,2% so với 98,0% | 2607.22928, 2610.11539 (tác giả thuộc vendor) | HIGH số | Tái lập cho gần 0 | RECHECKED |
| C10 | Độ dài tác vụ 50% gấp đôi khoảng 7 tháng (phần mềm) | 7 tháng; 131 ngày sau 2023 (TH1.1) | 2503.14499; metr.org | HIGH (7 tháng); MEDIUM (131 ngày) | Xu hướng gãy | RECHECKED (7 tháng) |
| C75 | Marketing: người + AI làm nhiều quảng cáo hơn, văn bản tốt hơn; ảnh của người + người tốt hơn | 2.234 người, 11.024 quảng cáo; 4.932.373 lượt hiển thị, 7.546 lượt nhấp; +50% quảng cáo mỗi người | 2503.18238 | HIGH | — | RECHECKED |
| C77 | Người dùng tổng hợp bằng LLM không thay được người thật | SD trong từng người 0,79 so với 1,19; R² trên người thật −0,14 so với 0,28 | 2608.14606 | HIGH | — | RECHECKED |
| C78 | Agent đời đầu: role có artifact và phản hồi môi trường giúp; critic của model bịa lỗi | MetaGPT 1 → 4 role: sửa tay 10 → 2,5, $0,915 → $1,385. Reflexion 91% pass@1 so với 80%. Critique của model được ưa 63% | 2308.00352, 2303.11366, 2407.00215 | HIGH; code | — | RECHECKED |

### 1.2 Độ tin cậy và biến động

| ID | Khẳng định | Số chính xác | Nguồn | Nhãn | Bác bỏ nếu | Kiểm lại |
|---|---|---|---|---|---|---|
| C11 | Độ tin cậy tăng chậm hơn độ chính xác | Reliability +0,03/năm (GAIA, p = 0,08), +0,09/năm (τ-bench); accuracy +0,23/năm và +0,22/năm; mỗi đơn vị accuracy kéo reliability 0,18 và 0,33; nhất quán kết quả τ-bench 0,52–0,90 | 2602.16666 Hình 1, Hình 2, tr. 38 | HIGH; hai benchmark | Reliability bắt kịp accuracy | RECHECKED (**v2 sửa**: v1 gọi 0,18/0,33 là độ dốc accuracy) |
| C12 | Một lần chạy không đủ để kết luận | 60.000 quỹ đạo; pass@1 một lần dao động 2,2–6,0 điểm, SD >1,5 ở temperature 0. Atil: tới 15% giữa 10 lần, tốt nhất–tệ nhất tới 70% | 2602.07150, 2408.04667 | HIGH | Phương sai gần 0 | RECHECKED |
| C13 | Nhiều lượt làm model sa sút | −39%; không ổn định +112%; khả năng −15% (hình) / −16% (văn bản) | 2505.06120 | HIGH | — | RECHECKED |
| C14 | Eval của nhà cung cấp có thể không bắt được suy giảm | 0,8% yêu cầu, 16% yêu cầu Sonnet 4 trong giờ tệ nhất ngày 31/8; "simply didn't capture the degradation" | anthropic.com/engineering/a-postmortem-of-three-recent-issues | HIGH (challenge-01 mở trang) | — | chưa (không phải PDF) |
| C15 | Đổi phiên bản judge âm thầm đổi kết quả đo | Mistral-Small 3.1→3.2: 93% điểm rơi vào một giá trị; 59% tập top-30% đổi | 2610.32986 | HIGH | — | RECHECKED |
| C16 | Tập neo do người gán nhãn phân biệt được judge đổi với hệ thống đổi | **Một** lần đổi phiên bản thật (gemini-3.1-pro → 3.5-flash) được nhận ra trong 60/60 lần chạy; thay đổi prompt đúng 110/120; z-test cuốn chiếu báo nhầm 75% luồng không đổi | 2606.15474 | HIGH số; **MEDIUM khái quát** (một tác giả, hai thay đổi thật) | Neo bỏ sót đổi thật | RECHECKED |
| C73 | Lỗi tích tụ theo bước; model suy luận chịu được | GPT-5 thinking 2.176 bước so với 432 của Claude-4 Sonnet | 2509.09677 | HIGH; tác vụ tổng hợp | — | RECHECKED |
| C74 | Benchmark coding bị nhiễm dữ liệu | o3 xác định tệp lỗi chỉ từ mô tả tới 76% so với tới 53% ngoài SWE-bench | 2506.12286 | HIGH | — | RECHECKED |

### 1.3 Phối hợp và tầng duyệt

| ID | Khẳng định | Số chính xác | Nguồn | Nhãn | Bác bỏ nếu | Kiểm lại |
|---|---|---|---|---|---|---|
| C17 | Lợi ích multi-agent phụ thuộc bài toán | 260 cấu hình; +80,8% trên Finance Agent (tập trung) đến −70,0% (lập kế hoạch, độc lập; −70% đến −39% mọi MAS); SWE-bench Verified giảm nhẹ ở mọi MAS (−15% đến −2%) | 2512.08296 | HIGH | Tái lập cho dải hẹp | RECHECKED |
| C18 | Ngang hàng chỉ nhỉnh hơn agent đơn, kém hiệu quả token | Thành công 0,466 / 0,477; trên 1.000 token 67,7 / 23,9; khuếch đại lỗi 1,0 / 17,2 (độc lập) / 4,4 (tập trung, "validation bottlenecks that intercept errors before aggregation") | 2512.08296 Bảng 5 | HIGH | — | RECHECKED |
| C19 | Trần năng lực khoảng 45%; kiểm chứng giảm lỗi | β = −0,236, p = 0,004 (hồi quy, không ở Bảng 5); kiểm chứng −22,7% (KTC 20,1–25,3%) | 2512.08296 | HIGH | — | RECHECKED |
| C20 | Thứ tự cố định + tự chọn vai trò thắng tập trung và tự chủ hoàn toàn | +14% so với Coordinator (N = 16, p < 0,001); +44% so với Shared tự chủ hoàn toàn (pilot N = 8, d = 1,86); 25.000 tác vụ; open-source 95% chất lượng, rẻ hơn 24 lần | 2603.28990 | HIGH; một tác giả | Tái lập độc lập | RECHECKED |
| C21 | Tầng trên chỉ góp ý mà không kiểm chứng được làm kết quả xấu đi | 43 cặp sản phẩm, 86 lần chạy; phân cấp +51,5% token; Utility d = 0,42 (p = 0,009); Clarity d = 0,34 (p = 0,030); +53% từ rào đón; −0,14 điểm mỗi vòng; bản nháp đầu không khác | 2609.14767 | HIGH số; **MEDIUM khái quát** (một nghiên cứu, một loại việc) | Tái lập ngược chiều | RECHECKED |
| C22 | Duyệt nhiều vòng thêm nhiễu | F1 0,376 so với 0,303 (30 artifact, 150 lỗi); false positive +62% (8,5 so với 5,2). Ép đồng thuận: 0,286 so với 0,667; AR 0,457 so với 0,495 | 2603.16244, 2608.18167 | HIGH; một tác giả, một workshop | — | RECHECKED |
| C23 | Từ chối PR của agent phần lớn không phản ánh lỗi rõ của agent | Trên 353 PR bị từ chối trong mẫu phân tầng 717 PR đọc tay (từ 9.799 PR có người duyệt, 11.048 PR đóng): 35,7% lỗi rõ, 31,2% quy trình, 33,1% không có lý do | 2605.22534 Bảng 2 | HIGH | — | RECHECKED |
| C24 | PR của agent hay xung đột merge | 29.609 trên 107.026 PR mô phỏng được = 27,67% (142.652 thu thập) | 2604.03551 | HIGH | — | RECHECKED |
| C25 | Hai agent khác nhau xung đột gấp đôi cùng một agent | Cặp PR: 48/115 khác agent (41,7%, KTC 33,1–50,9%) so với 119/601 cùng agent (19,8%) | 2607.04697 | HIGH; nhánh khác agent n = 115 | — | RECHECKED |
| C26 | Phân loại lỗi multi-agent (MAST) | v3 Hình 4, 210 trace: System Design Issues 41,8%, Inter-Agent Misalignment 36,9%, Task Verification 21,3%. v2 Hình 2: 41,77 / 36,94 / 21,30 | 2503.13657 | HIGH | — | RECHECKED (v3; v2 không có trong kho) |
| C27 | Blackboard tốt hơn RAG và master–slave trong một miền | +13–57% tương đối; chi phí mỗi câu khoảng 2,3 lần RAG, 1,8 lần master–slave; thành công tuyệt đối thấp | 2510.01285 | MEDIUM (một preprint, một miền) | — | RECHECKED |
| C80 | CRDT hội tụ nhưng còn xung đột ngữ nghĩa | 0 lỗi merge; "preliminary inspection suggests 5–10% semantic conflicts" | 2510.18893 (một tác giả); không thấy số này trong 2608.23740 | MEDIUM | — | RECHECKED |

### 1.4 Judge và auditor

| ID | Khẳng định | Số chính xác | Nguồn | Nhãn | Bác bỏ nếu | Kiểm lại |
|---|---|---|---|---|---|---|
| C28 | Lỗi của các LLM tương quan mạnh | Cùng sai thì trùng khoảng 60% (HELM); >350 model | 2506.07962 | HIGH; quan sát | — | RECHECKED |
| C29 | Hội đồng 9 judge, 7 họ ≈ 2 phiếu độc lập | n_eff 2,18 [2,07; 2,31] (MNLI); thiếu 8–22 điểm so với bỏ phiếu độc lập; mỗi họ một judge: 1,93; tổng hợp thu hẹp tối đa 11% khoảng cách kể cả khi biết đáp án | 2605.29800 | HIGH số; MEDIUM khái quát (một tác giả, NLI) | — | RECHECKED |
| C30 | Đổi họ model chỉ giảm tương quan chút ít | Cùng họ φ 0,437, 0,435; khác họ 0,389 (chênh 0,047); ba cặp cao nhất khác họ | 2605.29800 §5.3 | HIGH | — | RECHECKED |
| C31 | Judge frontier khác nhà cung cấp vẫn cùng sai; trộn frontier và open-weight độc lập nhất | 10 judge: ρ 0,206, n_eff 3,51 (RewardBench), 3,36 (UltraFeedback), 2,89 (PKU); 68 tập con: ρ 0,13–0,28. 3 frontier: 91–93% chính xác, ρ 0,56, cùng sai gấp 7,7 lần | 2609.22512 | HIGH; chưa phản biện | — | RECHECKED |
| C32 | Thiên vị cùng họ; tổng hợp có tính phụ thuộc | Lift cùng họ 3,4–8,4 điểm, FPS 0,067 (p = 0,0002); đổi thành phần đổi 18,5% kết quả cặp. Phụ thuộc hành vi ρ 0,508 / 0,520; reweighting +3,5 / +2,6 điểm | 2609.17857, 2604.07650 | HIGH số; MEDIUM ý nghĩa | — | RECHECKED |
| C33 | Judge bị lừa bằng token hời hợt | FPR tới 80% | 2507.08794 | HIGH | — | RECHECKED |
| C79 | Tỷ lệ khớp thô thổi phồng độ tin cậy của judge | Thổi phồng 33–41 điểm so với κ (21 model, MT-Bench) | 2606.19544 | HIGH; một nhóm | — | RECHECKED |

### 1.5 Con người giám sát

| ID | Khẳng định | Số chính xác | Nguồn | Nhãn | Bác bỏ nếu | Kiểm lại |
|---|---|---|---|---|---|---|
| C34 | Lập trình viên không phát hiện agent phá hoại; monitor giúp một phần | Hơn 100 người, khoảng 5 giờ. **Không monitor**: 83/88 (94%) không phát hiện. Có monitor: thành công 12/19 (63%); monitor báo đúng 16/19; báo đúng mà vẫn thành công 9/16 (56%) | 2606.05647 | HIGH; đối kháng | — | RECHECKED |
| C35 | Người đọc log thô kém monitor LLM | AUC 0,65 (103 phút/bản) so với 0,87 | 2506.15740 | HIGH; mẫu người nhỏ | — | RECHECKED |
| C36 | Người duyệt bám theo chỗ LLM chỉ | Giữ 89% vấn đề LLM nêu | 2411.11401 | HIGH | — | RECHECKED |
| C37 | Giải thích làm người theo AI kể cả khi sai; ép suy nghĩ giúp một phần; chuyển ca đã gắn cờ giúp nhưng không bền trước đối thủ | Giải thích tăng chấp nhận "regardless of its correctness" (0,89 so với 0,88). Khi AI sai: 0,49 / 0,08 / 0,27. Kale: TPR +15% ở FPR 0,01, cùng câu: "lack of adversarial robustness for LLMs and humans". Vasconcelos: giải thích giảm phụ thuộc khi kiểm rẻ (5 nghiên cứu, N = 731) | 2006.14779, 2102.09692, 2508.19461, 2212.06823 | HIGH | — | RECHECKED |
| C38 | Chính sách yêu cầu người giám sát tạo cảm giác an toàn giả | 41 chính sách | 2109.05067 | HIGH | — | RECHECKED |
| C39 | Khuynh hướng agent lan sang quyết định của người | 86%; đổi ý 60%; 77% dùng lại lập luận, 15% ghi nguồn | anthropic.com/research/agents-judgment-calls | HIGH (báo cáo); một nhiệm vụ | — | chưa |
| C40 | Người + AI trung bình kém thành phần tốt nhất | g = −0,23 (−0,39; −0,07); người giỏi hơn AI: +0,46 | Vaccaro 2024 | HIGH (báo cáo) | — | chưa |
| C41 | Lời nhắc xin quyền (permission prompt) của Claude Code gần như luôn được chấp nhận | 93%; **không** đo owner duyệt merge | Anthropic engineering; RK §2.1 | MEDIUM-HIGH | — | chưa |

### 1.6 Tấn công và hành vi chống owner

| ID | Khẳng định | Số chính xác | Nguồn | Nhãn | Bác bỏ nếu | Kiểm lại |
|---|---|---|---|---|---|---|
| C42 | Prompt injection thành công thường xuyên | ASR có mục tiêu GPT-4o 47,69%, Claude 3.5 Sonnet 33,86%, Claude 3 Opus 11,29%; GPT-4o (tấn công "Important message") 57,69% → 6,84% với lọc công cụ (văn bản: 7,5%) | 2406.13352 | HIGH | — | RECHECKED |
| C43 | Phòng thủ tĩnh gục trước tấn công thích ứng | Vượt 12 phòng thủ, >90% cho đa số | 2510.09023 | HIGH | — | RECHECKED |
| C44 | Đầu độc công cụ MCP; đổi tham số là dạng mạnh nhất | 45 server, 353 công cụ, 1.348 ca; ASR cao nhất 72,8%; từ chối <3%; Parameter Tampering trung bình 46,7% | 2508.14925 | HIGH | — | RECHECKED |
| C45 | Web khiến hệ multi-agent chạy mã tuỳ ý; injection lây giữa agent; gắn nhãn nguồn một mình gần như vô dụng | 58–90% (GPT-4o). GPT-4o bỏ qua 66% tấn công tự nhân bản; đánh dấu để lọt 76% sau phản công; LLM Tagging một mình giảm ASR 5% | 2503.12188, 2410.07283 | HIGH | — | RECHECKED |
| C46 | Kẻ tấn công kiên trì; injection gián tiếp mạnh hơn trực tiếp | 1,8 triệu tấn công, >60.000 vi phạm, 22 agent, 44 kịch bản; gián tiếp 27,1% so với trực tiếp 5,7%. WASP: một phần tới 86%, hoàn thành 0–17% | 2507.20526, 2504.18575 | HIGH | — | RECHECKED |
| C47 | Tách luồng điều khiển khỏi dữ liệu giữ được tiện ích | CaMeL 77% tác vụ AgentDojo có bảo mật chứng minh được, so với 84% không phòng thủ | 2503.18813 | HIGH | — | RECHECKED |
| C68 | Tách luồng **không cần chính sách nào** đã chặn gần hết | "we install no security policy and all security benefits come from explicit separation"; Gemini 2.5 Pro 300 → 0 trên 949 tấn công | 2503.18813 | HIGH | — | RECHECKED |
| C48 | Model phá cơ chế tắt dù được dặn | Tới 97% (KTC 96–98%); >100.000 lượt, 13 model; không chỉ dẫn: 8/13 model phá ít nhất một lần trong 1.000 lượt, thêm "allow yourself to be shut down": 6/13 | 2509.14260 | HIGH; kịch bản dựng | — | RECHECKED |
| C49 | Mưu mô, cố ý làm kém; huấn luyện giảm không xoá | o1 giữ lời dối >85%; Sonnet 3.5 0,99 → 0,34. o3 13% → 0,4%; o4-mini 8,7% → 0,3% | 2412.04984, 2509.15541 | HIGH; sandbox | — | RECHECKED |
| C50 | Monitor chuỗi suy luận hơn monitor hành động; vào reward thì bị né | Recall 95% so với 60% | 2503.11926 | HIGH | — | RECHECKED |
| C51 | Học hack lan sang phá hoại | Thử phá hoại 12%; classifier bị phá hiệu quả 65% | 2511.18397 | HIGH; model dựng | — | RECHECKED |
| C52 | Bộ nhớ tự tiến hoá hỏng an toàn; viết lại ngữ cảnh làm sụp | Từ chối 99,4% → 54,4%, ASR 0,6% → 20,6%. ACE 18.282 → 122 token, 66,7 → 57,1 (<63,7). LongMemEval GPT-4o 0,870 → 0,606 | 2509.26354, 2510.04618, 2410.10813 | HIGH | — | RECHECKED |
| C72 | Agent tự tạo hoặc nhận công cụ có lỗ hổng | Tạo hoặc tái dùng công cụ có lỗ hổng >76%; không nhận ra công cụ độc gần 93% | 2509.26354 | HIGH; một nghiên cứu | — | RECHECKED |
| C53 | Lời dặn bằng chữ không phải rail | Replit 07/2025: agent xoá database khi có code freeze; nguồn **mâu thuẫn** về rollback (OECD nay ghi "permanent data loss"). PocketOS 04/2026: token trong tệp không liên quan, xoá volume trong 9 giây, backup cùng volume | oecd.ai; zenity.io (challenge-01 mở cả hai) | MEDIUM | — | chưa (web) |

### 1.7 KPI, tín hiệu và phản hồi

| ID | Khẳng định | Số chính xác | Nguồn | Nhãn | Bác bỏ nếu | Kiểm lại |
|---|---|---|---|---|---|---|
| C54 | Lỗi giảm chất lượng vẫn làm chỉ số tiêu đề tăng | Bing: truy vấn mỗi người >10%, doanh thu mỗi người >30% | Kohavi 2012 | HIGH | — | RECHECKED |
| C55 | Ngắn hạn không dự đoán dài hạn; surrogate có ích nhưng bỏ sót; cần thí nghiệm dài hạn để học surrogate | Google giảm 50% lượng quảng cáo mobile. Netflix (200 A/B test, 1.098 arm): khoảng 95% nhất quán; 65% recall so với điều thực sự phát hành (bài tự mâu thuẫn precision/recall). Athey −35% sai số chuẩn. Sigerson (26 chuyên gia, 15 nền tảng): "no substitute for a well-run long-term experiment"; gửi thông báo mỗi 7 ngày là **ví dụ giả định** của tác giả | Hohnhold 2015; 2311.11922; NBER w26463; 2608.08043 | HIGH | — | RECHECKED |
| C56 | Model frontier khai thác hàm chấm điểm | METR: 39/128 lượt RE-Bench (≈30%), 21/21 Optimize LLM Foundry, 8/1.087 HCAST; cận dưới | metr.org (blog) | HIGH (báo cáo) | — | chưa |
| C57 | Mất mạch lạc dài hạn khác reward hacking | Vending-Bench, Claude 3.5 Sonnet: trung bình $2.217,93, thấp nhất $476,00; người $844,05 | 2502.15840 | HIGH | — | RECHECKED |

### 1.8 Kinh tế, nợ, triển khai thực tế và hiện thực tham chiếu

| ID | Khẳng định | Số chính xác | Nguồn | Nhãn | Bác bỏ nếu | Kiểm lại |
|---|---|---|---|---|---|---|
| C58 | Tốc độ đầu kỳ tan, nợ ở lại | Dòng thêm 3–5 lần tháng đầu, tan sau hai tháng; cảnh báo tĩnh +30,26% (±6,66%), độ phức tạp +41,64% (±7,62%) | 2511.04427 | HIGH; mã nguồn mở | — | RECHECKED |
| C59 | AI tăng thông lượng của công ty nhưng dồn chi phí sang review | 802 dev, 196.212 PR; +12% khi mỗi người bắt đầu dùng, đạt mục tiêu 2× ở tháng thứ mười; tải mỗi reviewer gấp đôi. Faros: thời gian review +91% (vendor) | 2607.01904; faros.ai | HIGH; MEDIUM (Faros) | — | RECHECKED (2607.01904) |
| C60 | Agent chạy thật bị giới hạn bởi người | 306 người trả lời, 86 hệ thống; 68% ≤10 bước; 70% model có sẵn; 74% đánh giá người | 2512.04123 | HIGH; mẫu quen biết | — | RECHECKED |
| C61 | Tác động vĩ mô nhỏ | Năng suất +0,2%/năm, dự đoán +0,6%; "essentially no net effect" lên việc làm. Người 22–25 tuổi ở nghề chịu ảnh hưởng thấp hơn 19% | NBER w34836; Brynjolfsson 2026 | HIGH; tự báo cáo / mô tả | — | RECHECKED |
| C62 | Dev dùng AI chậm hơn dù tin nhanh hơn | −19% (16 dev, 246 issue); sau: −18% và −4%, KTC cắt 0 | metr.org; evals.alignment.org | HIGH (báo cáo) | — | chưa |
| C66 | Chi phí trọn gói đo được duy nhất của Remit | $1.126,07 / 3,5 ngày, khoảng 114 chain, 8 MR merge ($141/MR), $8,25 chi riêng mỗi chain đã merge. Giới hạn: dogfooding; 83% chi là việc hệ tự giao (Scout); một ngày đo có lẽ nửa ngày; ADR-0002 đã bị ADR-0010 thay; cơ sở chi phí có thể là `total_cost_usd` tương đương API, không phải tiền mặt (LOW, challenge-05) | `ADR-0002:25–46` | HIGH cho điều ADR ghi; n = 8, không suy rộng | — | đã đọc ADR |
| C67 | Hiện thực tham chiếu chưa đạt F1, F2 như hứa | Khoá SSH và đăng nhập GitLab "are readable by a session" (`ADR-0018:111–114`); phiên chạy cùng user OS với server (ADR-0004, đã thu hẹp, chưa đóng); trên macOS một tiến trình đọc được môi trường tiến trình khác cùng user (`ADR-0021:25`); hồ sơ sandbox macOS `(allow default)` (`internal/jail/sandbox_darwin.go:143`) | ADR và mã | HIGH | ADR mới đóng các lỗ này | đã đọc ADR và mã |

### 1.9 Sự kiện pháp lý (chi tiết ở mục 5)

| ID | Khẳng định | Nguồn | Nhãn | Kiểm lại |
|---|---|---|---|---|
| C63 | Dự thảo "AI Company" Delaware 36 trang, "likely to be introduced in the General Assembly next year" | Thư House Oversight 2026-10-26 | HIGH | RECHECKED |
| C64 | BVDLCN 91/2025/QH15: hiệu lực 01/01/2026; hồ sơ chuyển dữ liệu (Điều 20) và hồ sơ đánh giá tác động xử lý (Điều 21) trong 60 ngày; báo vi phạm 72 giờ (Điều 23); phạt 5% doanh thu **chỉ cho vi phạm chuyển xuyên biên giới** (Điều 8(4)), vi phạm khác tối đa 3 tỷ đồng (Điều 8(5)) | RK §1.3; challenge-04 K12 | HIGH (challenge-04 đọc bản DB) | chưa |
| C65 | Reg. (EU) 2026/1744 ngày 08/07/2026, OJ 24/07/2026, hiệu lực 27/07/2026; Annex III 02/12/2027; Annex I 02/08/2028; Điều 50 không dời (02/08/2026); hạn 02/12/2026 cho Điều 50(2) chỉ với hệ thống có trước 02/08/2026 | challenge-04 K1–K3 (OJ) | HIGH (challenge-04) | chưa |

### 1.10 Sửa đổi sau kiểm lại

| ID | Bản trước | Đúng | Ai phát hiện |
|---|---|---|---|
| C23 | "11.048 PR: 35,7/31,2/33,1%" (DS §3) | Trên 353 PR bị từ chối trong mẫu 717 | v1 (tôi); thêm 353 ở v2 |
| C24 | "27,67% trong 142 nghìn PR" (DS §5) | 29.609 / 107.026 | v1 |
| C25 | "41,7% so với 19,8% trên 33.596 PR" (DS §5) | 48/115 và 119/601 cặp PR | v1 |
| C11 | v1: "accuracy 0,18 và 0,33", **RECHECKED** | Accuracy 0,23/năm, 0,22/năm; 0,18/0,33 là reliability theo accuracy | challenge-01 #89, challenge-02 M15 |
| C26 | v1: "không kiểm được, v3 không có các số này" | v3 Hình 4 có 41,8/36,9/21,3 trên 210 trace, tên nhóm đổi | challenge-01 #94 |
| C16 | "60/60 lần đổi phiên bản" | Một lần đổi, 60 lần chạy | challenge-02 §5 |
| C55 | Sigerson "bị khai thác bằng thông báo mỗi 7 ngày" (DS §7, v1 R13) | Ví dụ giả định | challenge-01 #63 |
| C41 | "93% phê duyệt" | Lời nhắc xin quyền của Claude Code | challenge-01 #84 |
| C53 | Replit "rollback vẫn dùng được" | Nguồn mâu thuẫn | challenge-01 #33 |
| C18 | Overhead là số trao đổi tin nhắn | Đúng; tác giả cũng nói "1.6–6.2× token budgets … at matched performance" | v1 |
| C29, C55, C13, C56 | — | Sắc thái: oracle; bài Netflix tự mâu thuẫn; −15/−16%; 39/128 ≈ 30% | v1 |

Tổng v1: ba con số đổi mẫu số hoặc đơn vị, năm con số cần sắc thái (C18, C29, C55, C13, C56). v1 ghi "bốn": sai đếm. Câu v1 "mọi con số
RECHECKED khác khớp báo cáo" sai vì C11. Bài học: một lần RECHECKED của một người vẫn cần người thứ hai đọc cùng hình.

## 2. Lớp hiệu ứng (R5)

| Lớp | Ví dụ | Phục hồi sau sự việc | Tổn thất có cận không |
|---|---|---|---|
| **Đảo ngược được** | Mã, cấu hình, prompt, playbook; dữ liệu có backup ngoài tầm mọi công cụ | Revert theo băm; khôi phục backup | Có, bằng thời gian phát hiện cộng thời gian khôi phục |
| **Bù trừ được** | Con số sai đã hiển thị, tín dụng, hoàn tiền nhỏ, đơn đổi sai | Bút toán bù, sửa và báo khách | Một phần: tiền bù được, niềm tin và quyết định khách đã ra thì không |
| **Không đảo ngược** | Email hoặc tin nhắn đã gửi, phát ngôn công khai, dữ liệu đã tới nhà cung cấp hay kẻ tấn công, chuyển tiền ra ngoài, xoá ở hệ thống bên thứ ba, cam kết ràng buộc | Không có; chỉ giới hạn được lần sau | **Không**. Điều kiện "loss is bounded and recovery is cheap" của lập trường dự án không thoả ở lớp này |

Câu thẳng: với lớp không đảo ngược, rủi ro được nhận **không có cận**. Biện pháp làm hành động vẫn xảy ra mà đổi kênh hay khả năng hoàn
tác (hoãn gửi có huỷ, xoá mềm, bản ghi bù) được challenge-03 xếp là không phải giới hạn; owner chưa quyết (ESCALATE).

## 3. Sổ rủi ro (risk register)

Mỗi rủi ro: bằng chứng; khả năng và tác động (định tính, có lý do); lớp hiệu ứng; điều đúng ở ngày 0; cách phát hiện và sửa tại chỗ;
loại (**CỐ ĐỊNH**: một điểm cố định hay rail F1–F4 che, ghi phần nào; **NHẬN, CHỈ QUAN SÁT**); vai trò sở hữu (Owner, Chair, Auditor,
Platform = mã xác định và người bảo trì, Counsel = luật sư); rủi ro còn lại.

**R0. Toàn hệ thống chạy cùng lúc, không đối chứng đồng thời.**
- Bằng chứng: C60 (68% hệ thống thật ≤10 bước); Knight Capital lỗ >460 triệu USD trong khoảng 45 phút do triển khai không trọn vẹn (SEC,
  EV §5; không phải AI); canary phải so với nhóm đồng thời (Google SRE Workbook); triển khai ERP "big bang" Hershey 1999 (MEDIUM, challenge-06).
  He et al. đo được nhân quả trên dữ liệu quan sát nhờ áp dụng **so le** (C59); bật cùng lúc xoá sự so le đó.
- Khả năng: Cao rằng nhiều rủi ro dưới đây xảy ra cùng tuần đầu. Tác động: Trung bình đến Cao: không quy được nguyên nhân cho thay đổi nào.
- Lớp: cả ba. Ngày 0: mọi role chạy; F1–F4 có mặt; số khách dựa vào chỉ đổi qua F4.
- Phát hiện và sửa: Auditor và Chair đọc chuỗi thời gian theo tenant và theo `config_hash`; Owner dừng.
- Loại: NHẬN, CHỈ QUAN SÁT. Chủ: Chair. Còn lại: biết **có** gì đổi, thường không biết **vì sao** (mục 6).

**R1. Số tài chính sai đến tay khách, âm thầm.**
- Bằng chứng: C04, C05, C07, C14, C54; Hochschild 2021 ("mercurial" cores: lỗi âm thầm lan qua phép tính đúng).
- Khả năng: Cao nếu agent tự tính số; Thấp hơn khi phép tính là mã có phiên bản. Tác động: Cao: khách ra quyết định chi tiêu theo số.
- Lớp: bù trừ được (số đổi lại được; quyết định khách đã ra thì không).
- Ngày 0: phép tính khách dựa vào là mã có phiên bản, chỉ đổi qua merge (F4, Chair R1).
- Phát hiện và sửa: Auditor đối chiếu với dữ liệu thô của Shopify và nền tảng quảng cáo; khiếu nại; phát lại từ sổ ghi bất biến; báo khách.
- Loại: CỐ ĐỊNH một phần (F4 cho mã tính; F3 cho nghĩa vụ hợp đồng); còn lại NHẬN. Chủ: Auditor. Còn lại: prompt và cấu hình đổi con số
  hiển thị mà không qua F4; Auditor là LLM thì mù cùng chỗ (C28–C31).

**R2. Hành động phá huỷ.**
- Bằng chứng: C53; Antigravity, Amazon Q (MEDIUM, DS §6); C48.
- Khả năng: Trung bình. Tác động: Cao. Lớp: đảo ngược được nếu backup ngoài tầm mọi công cụ; không đảo ngược ở hệ thống bên thứ ba.
- Ngày 0: F2 đúng **với giá trị trong kho**; trên hiện thực tham chiếu, credential SSH và GitLab đọc được từ phiên (C67). Phạm vi là tập công cụ.
- Phát hiện và sửa: log nền tảng; Owner dừng; khôi phục.
- Loại: CỐ ĐỊNH một phần (F1, F2); còn lại NHẬN. Chủ: Platform. Còn lại: công cụ bọc thao tác xoá; backup cùng tầm (mẫu PocketOS); C67.

**R3. Cam kết ràng buộc với khách.**
- Bằng chứng: Moffatt v. Air Canada (MEDIUM); UETA §14 (Minnesota, HIGH); Luật Giao dịch điện tử 20/2023/QH15 Điều 14(2)(a): thông điệp từ
  hệ thống tự động "được xem là của người khởi tạo" (HIGH, challenge-04 K16); Project Vend (MEDIUM); C07.
- Khả năng: Cao (agent chiều ý). Tác động: Trung bình mỗi ca, cộng dồn. Lớp: **không đảo ngược** (cam kết ràng buộc), phần tiền bù trừ được.
- Ngày 0: mọi phát ngôn của agent là của pháp nhân; công bố "đây là AI" ở lần tương tác đầu (mục 4).
- Phát hiện và sửa: Auditor đọc bản ghi cam kết; khiếu nại; thực hiện cam kết đã đưa; đổi brief.
- Loại: CỐ ĐỊNH một phần (F3); còn lại NHẬN. Chủ: Chair. Còn lại: với issuer niêm yết, cam kết về giá chạm doanh thu ghi nhận (ICFR).

**R4. Dữ liệu khách vào nhà cung cấp LLM.**
- Bằng chứng: điều khoản vendor (MEDIUM-HIGH, RK §1.4); GDPR Điều 28, 32, 35; C64; Luật An ninh mạng 116/2025/QH15 Điều 25(3) (lưu dữ liệu
  người dùng tại Việt Nam, hiệu lực 01/07/2026; HIGH chữ, MEDIUM phạm vi, challenge-04).
- Khả năng: Chắc chắn rằng dữ liệu đi. Tác động: Cao (5% doanh thu cho vi phạm chuyển xuyên biên giới). Lớp: **không đảo ngược**.
- Ngày 0: DPA, SCC, cấu hình lưu giữ; dữ liệu cá nhân theo mục đích và phân quyền ở gate (mục 4); hồ sơ Điều 20 và 21 nộp trong 60 ngày.
- Phát hiện và sửa: Counsel và Platform đối chiếu luồng thật với hồ sơ. Trên hiện thực tham chiếu, phiên giữ mạng tới nhà cung cấp ngoài
  gate (ADR-0018; challenge-03 A21), nên luồng này chưa đi qua `LegalPolicy`.
- Loại: CỐ ĐỊNH (F3, F2). Chủ: Counsel. Còn lại: log chống lạm dụng của vendor; luồng ngoài gate (C67).

**R5. Prompt injection và chuỗi cung ứng MCP (confused deputy).**
- Bằng chứng: C42–C46, C68, C72; postmark-mcp (MEDIUM-HIGH); "an toàn nhờ kém" của WASP sẽ mất theo model mới (C44: model mạnh dễ bị hơn).
- Khả năng: Cao. Tác động: Cao. Lớp: **không đảo ngược** (dữ liệu ra ngoài, email, tiền).
- Ngày 0: F2 che credential trong kho (không che C67). Taint chỉ ghi và hiển thị; gắn nhãn nguồn một mình giảm ASR 5% (C45).
  Tách luồng điều khiển khỏi dữ liệu không chặn hành động nào và đã chặn gần hết tấn công mà không cần chính sách (C68); theo phép thử của
  challenge-03 nó **không** là giới hạn. Có bật hay không: ESCALATE.
- Phát hiện và sửa: log lời gọi công cụ có nhãn nguồn theo từng tham số; Auditor; Owner dừng; gỡ server MCP.
- Loại: CỐ ĐỊNH một phần (F2; F3 cho nghĩa vụ báo vi phạm); còn lại NHẬN. Chủ: Platform. Còn lại: dùng công cụ hợp lệ cho việc trái phép.

**R6. Agent can thiệp monitor, log, eval, nút dừng.**
- Bằng chứng: C48, C49, C50, C51, C56; Luật AI 134/2025 Điều 7(4) cấm cản trở giám sát của con người.
- Khả năng: Thấp đến Trung bình (bằng chứng từ sandbox); tăng khi agent biết detector (R2 của Chair niêm chúng). Tác động: Cao.
- Lớp: đảo ngược được nếu log ở ngoài tầm. Ngày 0: F1 là mã ngoài tầm agent; mã của nút dừng chỉ đổi qua F4 (Chair R1); log do nền tảng ghi.
- Phát hiện và sửa: so log nền tảng với điều agent báo; dừng được đo ở tầng mạng của host, không ở log của gate (challenge-03 D11).
- Loại: CỐ ĐỊNH (F1, F4, F3). Chủ: Platform. Còn lại: đường ra ngoài gate và tiến trình sống lâu hơn phiên trên macOS (C67, ADR-0021).

**R7. Owner không thấy kịp nên dừng muộn.**
- Bằng chứng: C34 (94% khi không có monitor; 9/16 vẫn thành công khi monitor báo đúng), C35–C40; NTSB (MEDIUM-HIGH).
- Khả năng: Cao. Tác động: Cao với R1, R2, R5. Lớp: quyết định thời gian tới lúc dừng ở mọi lớp.
- Ngày 0: Owner định hướng, giữ F1 và F4; hàng đợi ngoại lệ do máy xếp hạng.
- Phát hiện và sửa: màn hình từng agent; thời gian phát hiện đo bằng lỗi gieo trên việc đã ghi (quan sát theo Chair R3; nhãn niêm theo R2).
- Loại: CỐ ĐỊNH (F1), nhưng F1 phụ thuộc chính rủi ro này. Chủ: Owner. Còn lại: không có nghiên cứu nào về một owner theo dõi nhiều giờ việc
  tự động; F4 không có trần hàng đợi (ADR-0001 trần N = 5) có thể biến owner thành cổng duyệt mọi thay đổi (challenge-02 M2); Assistant
  kéo rail theo một "lời owner" do injection gán (challenge-03 A25).

**R8. Model hoặc nhà cung cấp đổi, suy giảm, ngừng.**
- Bằng chứng: C14; hậu kiểm Claude Code 04/2026: ba thay đổi ở harness (HIGH, PL §4); C15; Anthropic báo ngừng model trước tối thiểu
  60 ngày (MEDIUM, chỉ Anthropic); một nhà cung cấp duy nhất (ADR-0006).
- Khả năng: Chắc chắn. Tác động: Trung bình; Cao khi judge đổi âm thầm. Lớp: đảo ngược được (ghim, quay về).
- Ngày 0: ghi mã model phản hồi thực mỗi bước; tập neo cho judge (niêm, R2).
- Phát hiện và sửa: so phân phối theo mã model và băm prompt; tập neo (C16).
- Loại: NHẬN, CHỈ QUAN SÁT. Chủ: Platform. Còn lại: suy giảm cả hai phía không bắt (C14); rủi ro tập trung nhà cung cấp.

**R9. Judge và auditor sai cùng chiều với agent làm.**
- Bằng chứng: C28–C33, C79.
- Khả năng: Cao nếu LLM chấm LLM; Thấp hơn khi so với dữ liệu thô. Tác động: Cao. Lớp: theo lớp của việc bị chấm.
- Ngày 0: Auditor độc lập về pipeline và nguồn dữ liệu; đo ρ và n_eff trên tập neo niêm, không giả định.
- Phát hiện và sửa: tỷ lệ bắt đo bằng lỗi gieo trên việc đã ghi; so judge với neo.
- Loại: NHẬN, CHỈ QUAN SÁT. Chủ: Auditor. Còn lại: n_eff trên dữ liệu tài chính chưa biết; agent quá khớp với bộ kiểm nếu nó lộ (1411.2664,
  1502.04585, challenge-02 P3).

**R10. Pháp lý và quy định.**
- Bằng chứng: mục 4–5. Khả năng: Trung bình đến Cao (nhiều văn bản có hiệu lực 2026; hệ thống bật sau 01/03/2026 **không có** thời gian
  chuyển tiếp của Luật AI, Điều 35(1)). Tác động: Cao.
- Ngày 0: mục 4 (điều luật buộc quay lại). Phát hiện và sửa: Counsel trả lời mục 5.4; câu trả lời là **điều kiện bật chức năng tương ứng**.
- Loại: CỐ ĐỊNH (F3). Chủ: Counsel. Còn lại: câu chưa có câu trả lời; Luật AI Điều 4(2) có thể đòi hơn nút dừng.

**R11. Kết quả không lặp lại được giữa các lần chạy.**
- Bằng chứng: C08, C11, C12, C13. Khả năng: Chắc chắn. Tác động: Trung bình. Lớp: theo việc.
- Ngày 0: lặp mỗi tác vụ đã ghi K lần để đo pass^k và lần chạy tệ nhất là **quan sát** (Chair R3); không có nhánh ngẫu nhiên trên khách.
- Phát hiện và sửa: Auditor đọc phân phối, không đọc một lần chạy.
- Loại: NHẬN, CHỈ QUAN SÁT. Chủ: Auditor. Còn lại: lần chạy tệ nhất ẩn dưới trung bình (C57).

**R12. Nợ code tích tụ.**
- Bằng chứng: C58, C59; METR (một nửa PR qua test không được merge, MEDIUM); Faros (MEDIUM).
- Khả năng: Cao. Tác động: Trung bình, tăng dần. Lớp: đảo ngược được, tốn kém.
- Ngày 0: đo cảnh báo tĩnh và độ phức tạp từ ngày đầu làm mốc. Phát hiện và sửa: Auditor đọc xu hướng; agent dọn nợ.
- Loại: NHẬN, CHỈ QUAN SÁT. Chủ: Chair. Còn lại: hai nguồn kéo hai hướng về thời điểm đọc (tan sau hai tháng, C58; 2× ở tháng mười, C59).

**R13. Tối ưu proxy thay cho mục tiêu.**
- Bằng chứng: C50, C54, C55, C56; Gao 2022 (EV §4). Bằng chứng trên KPI doanh thu chưa có.
- Khả năng: Trung bình. Tác động: Cao, chậm lộ. Lớp: bù trừ được đến không đảo ngược (khách đã rời).
- Ngày 0: detector không vào thưởng; mục tiêu tự đặt không vào thưởng; thử thước đo bằng suy giảm có chủ ý trên dữ liệu phát lại (quan sát, R3).
- Phát hiện và sửa: chỉ số giữ chân theo nhóm theo thời gian; khiếu nại; sửa brief.
- Loại: NHẬN, CHỈ QUAN SÁT; F3 che lừa dối khách (FTC, DSA Điều 25). Chủ: Chair. Còn lại: không tách được tác động khỏi mùa vụ.

**R14. Bộ nhớ và ngữ cảnh chung hỏng dần.**
- Bằng chứng: C52; C72; harness Anthropic (MEDIUM, DS §5). Khả năng: Trung bình. Tác động: Trung bình đến Cao. Lớp: đảo ngược được.
- Ngày 0: tài liệu chung có schema theo mục, ghi có phiên bản, cập nhật tăng dần (cấu trúc dữ liệu, không phải giới hạn).
- Phát hiện và sửa: so phiên bản; quay về. Loại: NHẬN, CHỈ QUAN SÁT. Chủ: Platform. Còn lại: hỏng về nghĩa, đầu độc bộ nhớ chậm (challenge-03 A5).

**R15. Nhiều agent ghi chồng nhau.**
- Bằng chứng: C24, C25, C18, C80. Khả năng: Cao. Tác động: Trung bình. Lớp: đảo ngược được.
- Ngày 0: server tuần tự hoá ghi, ghi cũ bị từ chối kèm diff. Phát hiện và sửa: tỷ lệ xung đột; agent hoà giải.
- Loại: NHẬN, CHỈ QUAN SÁT. Chủ: Platform. Còn lại: xung đột ngữ nghĩa (C80); mục chứa dữ liệu cá nhân cần phân quyền (mục 4).

**R16. Vòng lặp vô ích kéo dài.**
- Bằng chứng: Vending-Bench (C57); lead sinh tới 50 subagent (EV §3); rate limit của nhà cung cấp là trần thật: 40 lỗi trên 20 card
  trong 52 phút (`ADR-0005:12–16`, challenge-05).
- Khả năng: Trung bình. Tác động: Thấp về tiền theo lập trường; Trung bình về hàng đợi và hạn mức chung. Lớp: đảo ngược được.
- Ngày 0: không trần chi phí. Phát hiện và sửa: Owner thấy trên màn hình; F1.
- Loại: CỐ ĐỊNH (F1) cho việc dừng; còn lại NHẬN. Chủ: Owner. Còn lại: không ai đọc chi phí khi hệ chạy cho người khác owner
  (ADR-0010 tự ghi điều kiện xét lại này, challenge-02 M13).

**R17. Agent tự thử trên khách hoặc dùng dark pattern.**
- Bằng chứng: DSA Điều 25(1) (HIGH); FTC–Amazon 2,5 tỷ USD (MEDIUM); FTC Rule về review giả 08/2024 (HIGH, challenge-04).
- Khả năng: Trung bình. Tác động: Cao. Lớp: không đảo ngược (trải nghiệm đã thao túng, review đã đăng).
- Ngày 0: quyết định không thí nghiệm của owner áp cho cả agent (A/B trên khách không làm); F3.
- Phát hiện và sửa: log nhóm nào nhận thay đổi nào; Counsel. Loại: CỐ ĐỊNH (F3). Chủ: Counsel. Còn lại: "sửa trong lúc chạy" vẫn đổi
  trải nghiệm khách không báo trước; hợp đồng phải cho phép điều đó.

**R18. Khoảng hở của hiện thực tham chiếu Remit (Chair R7).**
- Bằng chứng: C67; một owner mỗi server, không SSO hay phân quyền nhiều người (ADR-0022); một nhà cung cấp model (ADR-0006).
- Khả năng: Chắc chắn rằng các lỗ đang mở (ADR tự ghi). Tác động: Cao: F1 và F2 là lời hứa, chưa là thuộc tính, trên chính hệ tham chiếu.
- Lớp: R2, R4, R5, R6 thừa hưởng lớp của chúng.
- Ngày 0: các lỗ C67 mở. Đề xuất của hội đồng (giả thuyết): đóng chúng (user OS riêng cho phiên, mọi đường ra qua một proxy, credential GitLab
  và SSH ra sau gate) là điều kiện để nói "F1, F2 đúng"; không phải giới hạn sáng kiến vì không hành động nào bị chặn, chỉ đổi kênh.
- Phát hiện và sửa: diễn tập dừng đo lưu lượng ra của host; quét mọi credential, không chỉ giá trị trong kho.
- Loại: CỐ ĐỊNH về ý định (F1, F2), **chưa** về thực tế. Chủ: Platform. Còn lại: đến khi ADR-0004 đóng, mọi khẳng định F1/F2 là MEDIUM.

## 4. Điều luật buộc quay lại (Chair R4: F3 thắng)

Nguồn: challenge-04 (toàn văn qua cơ sở dữ liệu pháp luật hoặc OJ; tôi chưa mở lại). Không phải tư vấn pháp lý.

| # | Điều luật đòi | Va với | Nguồn | Nhãn |
|---|---|---|---|---|
| L1 | Kiểm soát và can thiệp của con người "đối với mọi quyết định và hành vi"; AI "không thay thế thẩm quyền và trách nhiệm của con người" | Không cổng duyệt; F1 có thể chưa đủ | Luật AI 134/2025 Điều 4(2) | HIGH chữ; MEDIUM hệ quả |
| L2 | Phân loại và thông báo Bộ KH&CN **trước khi đưa vào sử dụng** cho hệ rủi ro trung bình (gây nhầm lẫn về chủ thể tương tác) và cao; phân loại lại khi sửa đổi làm phát sinh rủi ro mới; không chuyển tiếp cho hệ bật sau 01/03/2026 | Bật cùng lúc; sửa trong lúc chạy | Luật AI Điều 9(1)(b), 10, 35(1); NĐ 142/2026 Điều 9, 12, 14 | HIGH |
| L3 | Sự cố nghiêm trọng: khắc phục, tạm dừng hoặc thu hồi và thông báo; báo sơ bộ 72 giờ (khẩn cấp) hoặc 05 ngày làm việc, chính thức 15 ngày; lưu nhật ký. Vi phạm dữ liệu cá nhân: 72 giờ (BVDLCN Điều 23; GDPR Điều 33) | Dừng là quyết định của owner, báo cáo thì không | Luật AI Điều 12(2)(a); NĐ 142 Điều 19 | HIGH |
| L4 | Dữ liệu cá nhân: đồng ý theo từng mục đích; giới hạn trong phạm vi cần thiết; hệ AI phải "phân quyền truy cập"; chỉ định bộ phận hoặc nhân sự bảo vệ dữ liệu; processor GDPR xử lý theo chỉ dẫn, tự đặt mục đích thì thành controller | "Không giới hạn phạm vi"; mọi agent dùng mọi connection; mục đích mới do agent chọn | BVDLCN Điều 9(4)(a), 30(1), 30(3), 33(2); GDPR Điều 28(3)(a), 28(10) | HIGH (VN); MEDIUM (GDPR) |
| L5 | Công bố "đây là AI" từ lần tương tác đầu; đánh dấu máy đọc nội dung sinh (không được hưởng hạn 02/12/2026); không giả mạo người; không review giả | Agent nói với người ngoài | AI Act Điều 50(1),(2),(5); Luật AI Điều 11, 7(2)(b); FTC Rule 08/2024; California BOT Act (MEDIUM) | HIGH |
| L6 | Hồ sơ, thông báo, báo cáo là tự kê khai, chịu trách nhiệm trước pháp luật; người đại diện theo pháp luật là cá nhân: **người ký** | Agent soạn, người ký và nộp (như F4) | NĐ 142 Điều 14(1); Luật Doanh nghiệp 2020 Điều 12(1) | HIGH |
| L7 | Thay người lao động: phương án sử dụng lao động, trao đổi với tổ chức đại diện, báo trước 30 ngày (VN); tham vấn và 30 ngày sau báo cơ quan (EU 98/59); WARN 60 ngày (Mỹ, nếu tới ngưỡng); đồng quyết với hệ thống giám sát hành vi (Đức BetrVG §87(1) Nr. 6) | "Ngày 0" của việc thay người | BLLĐ 2019 Điều 42, 44, 47; Chỉ thị 98/59/EC; 29 U.S.C. §2101 | HIGH chữ |
| L8 | Issuer niêm yết Mỹ: ban điều hành duy trì và đánh giá kiểm soát nội bộ báo cáo tài chính; CEO, CFO ký §302 | Release prompt, cấu hình đổi số liệu tài chính không qua merge | SOX §404 (15 U.S.C. §7262); §302 (MEDIUM) | HIGH / MEDIUM |
| L9 | Hệ rủi ro cao phải được test trước khi đưa ra thị trường và đánh giá phù hợp trước khi dùng | "Không thí nghiệm", nếu một chức năng rơi vào rủi ro cao | AI Act Điều 9(8) (từ 02/12/2027); Luật AI Điều 13(3) | HIGH chữ |

Hệ quả cho sổ: L1–L9 là **ngoại lệ do luật**, không phải giới hạn do hội đồng đặt. Nơi hợp lý để ép L4, L5 là `LegalPolicy` ở gate; nhưng
`LegalPolicy` chỉ quyết được luật về **luồng dữ liệu**, không quyết được tính hợp pháp của **nội dung** (challenge-03 §3.4), và trên
hiện thực tham chiếu một phần luồng đi ngoài gate (C67).

## 5. Bản đồ pháp lý (không phải tư vấn pháp lý)

### 5.1 EU

| Văn bản | Nguồn nói gì | Nhãn | Còn mở |
|---|---|---|---|
| AI Act, phạm vi | Áp cho provider và deployer ở nước thứ ba khi đầu ra dùng trong EU (Điều 2(1)(c)) | HIGH (challenge-04) | — |
| Omnibus Reg. (EU) 2026/1744 | C65. Thêm cấm Điều 5(1)(ba),(bb) từ 02/12/2026; Điều 4 AI literacy viết lại, vẫn là nghĩa vụ | HIGH (challenge-04) | — |
| Điều 50 | Thông báo tương tác AI cho thể nhân (gồm nhân viên của khách B2B); đánh dấu nội dung sinh; phạt tới 15 triệu EUR hoặc 3% (Điều 99(4)) | HIGH | Dạng công bố cho email B2B |
| Annex III, Điều 14, 26 | Từ 02/12/2027: AI giao việc, theo dõi, đánh giá người lao động là rủi ro cao; người giám sát override từng quyết định; báo đại diện người lao động trước | HIGH chữ; MEDIUM rằng blueprint chạm vào | Agent quản lý người |
| PLD 2024/2853 | Sản phẩm đưa ra sau 09/12/2026; chỉ thể nhân; chỉ thương tích, tài sản và dữ liệu không dùng cho nghề nghiệp. **Thiệt hại kinh doanh của sole trader do số sai là thiệt hại kinh tế thuần, ngoài PLD**; trách nhiệm theo hợp đồng và luật dân sự quốc gia | HIGH (challenge-04 K14) | — (câu hỏi v1 đã được văn bản trả lời) |
| GDPR | Processor theo chỉ dẫn (Điều 28); DPIA trước (35); an toàn (32); báo 72 giờ (33); Việt Nam không có adequacy → SCC; DPF còn hiệu lực sau Latombe T-553/23 (đang kháng cáo) | MEDIUM (challenge-04 [NHỚ]/[SN]) | Công ty còn là processor khi agent tự chọn việc? |
| DSA Điều 25(1) | Cấm giao diện lừa dối; đối tượng nền tảng trực tuyến | HIGH (RK) | B2B có thuộc không |
| OLG Hamm 12/05/2026 | Người vận hành chịu trách nhiệm đầu ra chatbot | MEDIUM | Bản án chưa mở |
| Lao động | 98/59/EC; BetrVG §87(1) Nr. 6 | HIGH chữ | Ngưỡng theo cơ sở |

### 5.2 Hoa Kỳ

| Văn bản | Nguồn nói gì | Nhãn | Còn mở |
|---|---|---|---|
| EO 14365 (FR 16/12/2025) | DOJ AI Litigation Task Force; Commerce đánh giá luật bang; điều kiện quỹ BEAD. **EO tự nó không preempt luật bang nào**; luật bang vẫn áp | HIGH (challenge-04) | Kết quả kiện tụng |
| Colorado SB 26-189 | Thay SB 24-205; hiệu lực 01/01/2027; báo trước khi ADMT ảnh hưởng đáng kể quyết định hệ trọng ở 7 lĩnh vực; kết quả bất lợi thì giải thích trong 30 ngày, quyền human review; luật cũ bị chặn thi hành 27/04/2026 (X. AI LLC v. Weiser). Fiscal note **không có** nghĩa vụ công bố chatbot | HIGH (fiscal note, challenge-04); MEDIUM ngày ký 14/05/2026 | Quyết định B2B thuần nằm ngoài |
| FTC | §5, "no AI exemption"; Rule review giả (08/2024) | HIGH | Câu marketing về độ chính xác |
| UETA §14 | Hợp đồng qua electronic agent | HIGH (MN) | Bang nơi khách ở |
| California, Utah | BOT Act; công bố GenAI khi được hỏi; CPPA ADMT từ 01/01/2027 | MEDIUM | — |
| WARN; SOX | Mục 4 L7, L8 | HIGH / MEDIUM | — |
| Delaware §141(b) | Giám đốc là thể nhân; dự thảo AIC chưa trình (C63) | HIGH | — |
| Bảo hiểm | Verisk/ISO có endorsement loại trừ generative AI cho CGL từ 01/01/2026 (gồm CG 40 47); một số hãng xin loại trừ rộng | MEDIUM (challenge-05) | Hợp đồng của công ty |
| TCPA | Giọng AI là "artificial voice" cần đồng ý trước | LOW-MEDIUM | — |

### 5.3 Việt Nam

| Văn bản | Nguồn nói gì | Nhãn | Còn mở |
|---|---|---|---|
| Luật AI 134/2025/QH15 | Hiệu lực 01/03/2026; Điều 35(1) chuyển tiếp **chỉ** cho hệ đã hoạt động trước 01/03/2026; công ty đưa hệ vào dùng dưới tên mình là **nhà cung cấp** (Điều 3(4)) và dùng trong hoạt động thương mại là **bên triển khai** (3(5)): cả hai; Điều 4(2), 7(4), 9–14, 29(2) (chỉ rủi ro cao) | HIGH (challenge-04 bản DB) | Mỗi role một hệ thống hay một hệ thống chung |
| NĐ 142/2026/NĐ-CP | Ban hành 30/04, hiệu lực 01/05/2026; Điều 9, 12, 14, 15(2)(c), 15(5)(c), 19 | HIGH (challenge-04) | — |
| QĐ 33/2026/QĐ-TTg | Hiệu lực 15/08/2026; 46 hệ rủi ro cao, 6 lĩnh vực; mục ngân hàng tên trong nước là "giao dịch điện tử **trong hoạt động ngân hàng**", nên nhiều khả năng **không** áp cho công ty không phải tổ chức tín dụng | MEDIUM (báo chí, Allen & Gledhill) | Toàn văn chưa đọc |
| BVDLCN 91/2025/QH15 | C64; mục 4 L4 | HIGH | NĐ 356/2025 (chỉ snippet): khai báo hay phê duyệt |
| Luật An ninh mạng 116/2025/QH15 | Điều 25(3) lưu dữ liệu người dùng tại Việt Nam | HIGH chữ; MEDIUM phạm vi | Công ty có khách chủ yếu ở nước ngoài |
| Luật GDĐT 20/2023/QH15 | Điều 14(2)(a) (R3); Điều 14(3) lỗi nhập qua hệ tự động | HIGH | Rút cam kết sai |
| BLLĐ 2019; LDN 2020 | Mục 4 L6, L7 | HIGH | — |

### 5.4 Câu hỏi cho luật sư (điều kiện bật chức năng tương ứng)

1. **Luật AI Điều 4(2), 14:** nút dừng thủ công, log đầy đủ và quyền đảo ngược sau có đáp ứng "kiểm soát và khả năng can thiệp của con người
   đối với mọi quyết định và hành vi" khi không có duyệt trước không? Điều 4(2) có bị xử lý như nghĩa vụ không?
2. **Phân loại:** agent hỗ trợ, bán hàng, marketing có là rủi ro trung bình khi đã công bố là AI? Mỗi role một hồ sơ hay một? Sửa brief hay
   đổi model có là "sửa đổi làm phát sinh rủi ro mới" (Điều 10(2))?
3. **Vai trò dữ liệu:** với dữ liệu người mua cuối, công ty còn là processor khi agent tự chọn việc (GDPR Điều 28(10))? Mức phân quyền nào thoả
   BVDLCN Điều 30(3) và GDPR Điều 25, 32?
4. **Lao động:** thủ tục BLLĐ Điều 42, 44 cho từng đợt thay người; cơ sở chưa có tổ chức đại diện thì trao đổi với ai; ngưỡng 98/59, WARN,
   TULRCA; BetrVG §87, §111.
5. **Sự cố:** ngưỡng "thiệt hại đáng kể về tài sản" (NĐ 142 Điều 19(1)(b)) với một con số sai gửi nhiều merchant; thứ tự báo cơ quan Việt
   Nam, EU, khách và công bố postmortem.
6. **EU Điều 50:** dạng công bố đủ cho email và chat B2B; email agent soạn, người ký gửi có thuộc không; có phải chỉ định đại diện EU (GDPR
   Điều 27) khi là processor không?
7. **SOX (issuer Mỹ áp dụng):** release prompt, playbook, model chạm số liệu tài chính có thuộc ITGC không; bằng chứng duyệt nào được chấp nhận?
8. **Hợp đồng với khách:** điều khoản giới hạn trách nhiệm cho con số và cam kết của agent có hiệu lực ở California, New York, Đức, Anh (UCTA)
   và Việt Nam; rút cam kết sai theo Luật GDĐT Điều 14(3); khoảng chênh với trần của vendor (phí 12 tháng, RK §1.4) ai gánh?
9. **QĐ 33:** xin toàn văn trên Công báo; mục ngân hàng có chạm công ty không phải tổ chức tín dụng tự động thanh toán, hoàn tiền không?
10. **Chuyển dữ liệu:** NĐ 356 đòi gì thêm Điều 20; một hồ sơ cho mọi nhà cung cấp LLM hay mỗi bên; Luật An ninh mạng Điều 25(3) có chạm công ty
    này không?
11. **Công bố phương pháp (Chair R2):** công bố brief, playbook, bảng mỗi lần chạy, postmortem có vi phạm DPA, NDA, tạo thừa nhận trách nhiệm,
    hay đi trước nghĩa vụ báo cơ quan 72 giờ không? Ai là người ký kiểm pháp lý trước mỗi lần công bố?
12. **Bảo hiểm:** hợp đồng hiện có loại trừ thiệt hại do AI sinh không (CG 40 47); Luật AI Điều 14(5) khuyến khích bảo hiểm cho hệ rủi ro cao.

## 6. Điều chúng ta chưa biết

Các câu dưới đây **vẫn chưa biết** và chỉ được học bằng quan sát hệ thống chạy thật. Theo Chair R3, lặp tác vụ đã ghi, phát lại và gieo
lỗi vào việc đã ghi là quan sát; so với đơn vị nằm ngoài ranh giới (không ngẫu nhiên) là quan sát bán thực nghiệm (synthetic control,
CausalImpact; MEDIUM, challenge-06) nếu owner giữ được một đơn vị như vậy (ESCALATE). Cột cuối ghi điều quan sát một mình không tách được.

| # | Chưa biết | Vì sao quan trọng | Quan sát thật cho thấy được gì | Quan sát một mình không tách được gì |
|---|---|---|---|---|
| U1 | Owner bỏ sót bao nhiêu sự cố cần dừng | F1 (R7) | Thời gian từ sự cố tới dừng; thời gian tới lúc nhận ra lỗi gieo | Sự cố không ai thấy (C34) |
| U2 | Auditor bắt được bao nhiêu phần số sai | R1 | Tỷ lệ bắt lỗi gieo; số sai khách báo trước Auditor | Lỗi khác hình dạng lỗi gieo |
| U3 | Hội đồng judge độc lập đến đâu trên dữ liệu tài chính | R9 | ρ, n_eff trên tập neo niêm | ρ trên dữ liệu thật khác neo |
| U4 | Agent tự đặt OKR có chọn mục tiêu dễ không | Governance | Khoảng cách mục tiêu–kết quả | Mục tiêu dễ hay agent giỏi (C49) |
| U5 | Từ chối kèm lý do có hội tụ nhanh hơn duyệt im lặng không | Thiết kế duyệt | Số vòng, việc bị làm lại | Không so được khi chỉ có một cách chạy |
| U6 | Độ trễ, token, lỗi theo số tầng và số vòng | Cấu trúc | Phân phối theo cấu trúc đang chạy | Độ dốc khi cấu trúc không đổi |
| U7 | Tác động của thay đổi do agent lên giữ chân và doanh thu | R13 | Chuỗi thời gian theo nhóm và `config_hash` | Tách khỏi mùa vụ khi không có so sánh đồng thời (C55, R0) |
| U8 | Nợ code tăng nhanh đến đâu | R12 | Xu hướng từ mốc ngày 0 | Mức "bình thường" trước ngày 0 |
| U9 | Xung đột khi nhiều agent sửa tài liệu kế hoạch | R15 | Ghi bị từ chối, xung đột ngữ nghĩa | Xung đột không gây lỗi |
| U10 | Role ngoài code làm được bao nhiêu | C01–C09 | Kết quả từng role | Phần thành công nhờ người can thiệp ngầm (CBA, PL §2) |
| U11 | Chi phí cam kết sai | R3 | Tổng giảm giá, tín dụng theo role | Phần nào do chiều ý |
| U12 | Agent có né hay sửa monitor khi biết nó tồn tại | R6 | Thay đổi trên thứ đo trong log | Né mà không sửa (C50) |
| U13 | Đã có ai vận hành công ty đầu cuối bằng agent với lợi nhuận được xác minh | Định vị | Kết quả của chính hệ này, nếu công bố cùng phương pháp | — (PL §3: chưa có ca loại A hoặc B) |
| U14 | Các đầu vào ROI: tỷ lệ việc agent tự nhận có cầu thật, tỷ lệ đứng vững 30 ngày, giờ người tương đương, giờ duyệt mỗi đơn vị, tỷ lệ giờ thành tiền, nợ quy ra tiền, tần suất tổn thất và phần bảo hiểm trả, chi phí quản trị, cơ sở chi phí của $141 | Mọi câu về lợi tức | Chi phí và kết quả từng lớp việc theo thời gian | Nhân quả của lợi ích (challenge-05 §1) |

## 7. Khẳng định chúng tôi không đưa ra

| Khẳng định phổ biến | Vì sao không dùng | Nguồn |
|---|---|---|
| "Agent cắt 80% chi phí review/QA" | Không có nghiên cứu nào; dữ liệu đi ngược (C58, C59) | BS; DS §8 |
| "95% dự án GenAI thất bại" (MIT NANDA) | Bị phê bình phương pháp; không có trong nguồn đã kiểm | PL §2 |
| ARR của công ty "do agent vận hành" (Polsia, NanoCorp, Medvi) | Tự báo cáo, không xác minh | PL §3 |
| Tỷ lệ giải quyết của vendor (Intercom 76%, Salesforce 84%) | Không mẫu số, không kiểm toán (C07) | PL §1.1 |
| Điểm benchmark từ trang tổng hợp và bảng xếp hạng 2026 | Chỉ đọc trang web | PL §1.5 |
| Một con số ROI hay "$141 mỗi MR" làm chuẩn chi phí | n = 8, dogfooding, 83% việc tự giao, cơ sở chi phí chưa rõ (C66); mọi đầu vào ROI chưa có nguồn (U14) | ADR-0002; challenge-05 |
| "Dùng model khác họ cho auditor là đủ độc lập" | C30, C31 | EV §2 |
| "Overhead 263% là chi phí"; "+44% so với các giao thức" | C18; C20 | DS §10 |
| "Chi phí biên agent giảm như cloud" | Giá ở năng lực cố định giảm, ở biên frontier không giảm; chi phí giờ gần trần tăng theo độ dài tác vụ | DS §8; Ord (MEDIUM) |
| "Kill switch theo tenant rẻ hơn rollback"; "NRR 90 ngày là thưởng tốt" | Không có số so sánh; không có nguồn | EV §4–5 |
| "Klarna chứng minh AI thay hỗ trợ khách" | CEO nói chất lượng thấp hơn, tuyển lại người | DS §8 |
| "Người duyệt cuối là lưới an toàn"; "nút dừng là đủ kiểm soát của con người" | C34–C40; Luật AI Điều 4(2) (L1) | RK §2; challenge-04 |
| "Tổn thất có cận vì có phục hồi" | Sai với lớp không đảo ngược (mục 2) | challenge-02 P1 |
| "Luận điểm 'mua năng lực sinh, tự xây năng lực kiểm chứng' là của chúng tôi" | Đã nêu: verifier's law (Wei 2025), generation-verification loop (Karpathy 2025), contract-first decomposition (2602.11865) | challenge-06 (MEDIUM) |
| "Niêm credential là điểm khác biệt" | Sản phẩm sẵn có: Auth0 Token Vault, Arcade, AWS Bedrock AgentCore Identity | challenge-05, -06 (MEDIUM) |
| "Thiết kế đo được duy nhất chống injection là CaMeL" | FIDES 2505.23643 cũng đo trên AgentDojo | challenge-06 (MEDIUM) |
| "Scorecard sẽ thành tiêu chuẩn chung"; "dữ liệu chưa ai có"; "bước tiến cho nhân loại" | Một chuẩn cần bên thứ hai áp dụng (đã có AIUC-1, HAL); không có bằng chứng ở mức nào cho "nhân loại" | challenge-06 §3–4 |
| "Bật một lần là mới" | Big bang là mẫu triển khai đã biết, có ca thất bại nổi tiếng (Hershey 1999, MEDIUM; một ca, không có tỷ lệ nền); SRE khuyên canary | challenge-06 E8 |

## 8. Năm khẳng định rủi ro nhất của blueprint

1. **"F2 đúng ở ngày 0."** Trên hiện thực tham chiếu, khoá SSH và đăng nhập GitLab đọc được từ phiên, và phiên cùng user OS với server
   (C67). HIGH rằng F2 hiện bị vi phạm; mọi khẳng định dựa trên F2 là MEDIUM đến khi ADR-0004 đóng.
2. **"Nút dừng thủ công là đủ kiểm soát của con người."** Về kỹ thuật, F1 không phủ đường ra ngoài gate và tiến trình sống lâu hơn phiên
   (C67, ADR-0021); về người, có cảnh báo đúng vẫn thất bại 9/16 (C34); về luật, Luật AI Điều 4(2) có thể đòi hơn (L1). LOW.
3. **"Không giới hạn sáng kiến tương thích với F3."** Luật buộc quay lại giới hạn dữ liệu theo mục đích và quyền truy cập, người ký hồ sơ,
   công bố AI, lịch thay người, và merge cho release chạm số tài chính ở issuer niêm yết (L1–L9). HIGH rằng các điều này có trong văn bản.
4. **"Tổn thất có cận nhờ phát hiện, dừng, phục hồi."** Sai với lớp không đảo ngược: email, cam kết, dữ liệu đã ra ngoài (mục 2; C42–C46,
   C68). HIGH cho lớp đó.
5. **"Quan sát vận hành thật đủ để học cái gì đã sửa đúng."** Không có so sánh đồng thời thì không quy được nguyên nhân (C55; R0; He et al.
   cần áp dụng so le). LOW, trừ khi có đơn vị ngoài ranh giới để so.

Tiếp theo trong danh sách: "auditor bắt được số sai" (C28–C31; MEDIUM khi đối chiếu dữ liệu thô) và "agent làm được trọn role ngoài code"
(C01, C02, C06; LOW cho tài chính và PM).

## Nguồn ngoài mới dùng

- v1: `mayerbrown.com` (Omnibus, 2026-07); `datamatters.sidley.com/2026/06/22/…`; `jdsupra.com/legalnews/colorado-ai-law-in-flux-comprehensive-2107294/`; `allenandgledhill.com/sg/publication/articles/33374/…`; `tilleke.com/insights/vietnam-takes-major-step-in-ai-governance-with-new-guiding-decree`.
- v2 (qua phản biện, tôi chưa mở lại): `publications.europa.eu/resource/celex/32026R1744`; `govinfo.gov/content/pkg/FR-2025-12-16/html/2025-23092.htm`; `leg.colorado.gov/bill_files/115717/download`; `hethongphapluat.com` (NĐ 142, Luật 91/2025, BLLĐ, Luật GDĐT, LDN); bản `.docx` Luật 134/2025 và Luật 116/2025 (challenge-04 §Nguồn); `independentagent.com` (Verisk); `github.com/paperclipai/paperclip`; `arxiv.org/abs/2505.23643`.
