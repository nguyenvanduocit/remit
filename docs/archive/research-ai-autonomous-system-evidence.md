# Kiểm chứng bằng nguồn ngoài: hệ thống AI tự động toàn diện

> Kiểm chứng các khẳng định trong [`brainstorm-ai-autonomous-system.md`](brainstorm-ai-autonomous-system.md) bằng nguồn
> ngoài. Ngày chạy: 2026-11-01. Nhãn tin cậy: **HIGH** (nguồn gốc, đã kiểm), **MEDIUM** (suy ra hoặc nguồn một phía),
> **LOW** (phỏng đoán, cần kiểm thêm).

## Cách làm và phạm vi

Bốn lần chạy workflow `deep-research` (tìm kiếm song song, tải nguồn, rút claim, mỗi claim cho 3 verifier phản biện),
cộng với các lần đọc trực tiếp nguồn gốc của người viết báo cáo. Mỗi mục ghi rõ nguồn là workflow hay đọc tay.

| Lần chạy | Hướng | Agent | Claim đã verify | Ghi chú |
|---|---|---|---|---|
| 1 | ứng dụng thử nghiệm và lỗi tương quan của LLM judge | 102 | 23 giữ lại | Ba hướng còn lại gần như trống vì câu hỏi quá rộng |
| 2 | Cấu trúc multi-agent | 102 | 25 verify, 19 giữ, 6 bác | Workflow báo 6 mục bị bỏ do ngân sách (`budgetDropped`) |
| 3 | Reward hacking và KPI | 102 | 20 giữ lại | Mảng SaaS không có claim nào qua verify |
| 4 | Sự cố và thực hành production | 106 | 25 verify, 21 giữ, 4 bác | Workflow báo 6 mục bị bỏ do ngân sách (`budgetDropped`); nhiều claim trùng nguồn |

Giới hạn áp dụng:
- Nhiều verifier chỉ đọc abstract hoặc bản tóm tắt vì không tải được PDF.
- Hầu hết bằng chứng là preprint hoặc bài của vendor, đo trên benchmark (NLI, coding, preference), **không** đo trên dữ
  liệu tài chính hay SaaS. Áp vào ứng dụng thử nghiệm là suy rộng.
- "Không có kết quả" nghĩa là chưa kiểm chứng được, không có nghĩa là sai.

## 1. Phạm vi thử nghiệm

Các số liệu kinh doanh riêng đã được lược khỏi bản công khai.

## 2. Auditor cùng họ model có mù cùng chỗ không (lỗi tương quan)

Kết luận: **có**, và dùng model khác họ chỉ giúp một phần. Workflow, hướng 2.

- Lỗi của LLM tương quan đáng kể; cùng kiến trúc và nhà cung cấp là các yếu tố được nêu. Trên một dataset, hai model cùng
  sai thì giống nhau 60%. Kim et al., ICML 2025, hơn 350 model, `arxiv.org/pdf/2506.07962`. **HIGH** (đối chiếu toàn văn);
  quan sát, không chứng minh nhân quả.
- Một hội đồng 9 judge từ 7 họ chỉ tương đương **khoảng 2 phiếu độc lập** (n_eff Kish 2,18 trên MNLI, 2,35 trên SNLI,
  2,48 trên AlphaNLI), kém bỏ phiếu độc lập 8–22 điểm phần trăm; judge tốt nhất đơn lẻ bằng hoặc hơn cả hội đồng. Các thuật
  toán tổng hợp có sẵn chỉ thu hẹp tối đa 11% khoảng cách. Kohli, 2026-05, `arxiv.org/pdf/2605.29800`. **HIGH** cho
  số liệu (đối chiếu toàn văn); **MEDIUM** về tính khái quát: một tác giả, workshop, chỉ NLI.
- Cặp **cùng họ** chỉ tương quan cao hơn trung bình **khác họ** một chút (OpenAI–OpenAI φ = 0,437; Meta–Meta φ = 0,435; trung
  bình khác họ φ = 0,389, chênh 0,047). Ba cặp tương quan cao nhất đều khác họ (Claude–Gemini 0,603; GPT-4o–Claude 0,588;
  Mistral–DeepSeek 0,564). Chọn mỗi họ một judge tốt nhất thì n_eff còn **1,93**. Cùng paper, §5.3. **HIGH** (toàn văn).
  Lưu ý: lần chạy đầu của workflow đã bác claim này (phiếu 1–2) nhưng văn bản gốc xác nhận từng con số; xem mục 8.
- 10 judge tương đương khoảng 3,5 judge độc lập (tương quan lỗi trung bình ρ = 0,206; n_eff = 3,51 trên 1.133 cặp
  RewardBench), n_eff 3,36 trên UltraFeedback và 2,89 trên PKU-SafeRLHF; trên 68 tập con có ít nhất 5 judge, ρ từ 0,13 đến
  0,28. Ba judge frontier khác nhà cung cấp đạt 91–93% độ chính xác nhưng ρ = 0,56, và cùng sai gấp **7,7 lần** mức kỳ vọng
  khi độc lập. Abstract nêu thẳng: "judges from different providers are about as correlated as judges from the same
  provider", và **trộn judge frontier với judge open-weight cho nhiều thông tin độc lập nhất cùng độ chính xác bỏ phiếu cao
  nhất** trong các tổ hợp đã thử. Hossain et al., 2026-09, `arxiv.org/pdf/2609.22512`. **HIGH** (toàn văn); chưa qua phản
  biện.
- Cùng họ có thiên vị riêng: judge ưu ái output cùng họ (lift 3,4–8,4 điểm, FPS toàn cục 0,067, p = 0,0002; 4 họ open-weight,
  9.312 lượt đánh giá), và đổi thành phần hội đồng làm đổi 18,5% kết quả so sánh cặp. `arxiv.org/pdf/2609.17857`.
  Sự phụ thuộc hành vi giữa 18 LLM thuộc 6 họ liên quan tới thiên vị tán thành quá mức (ρ = 0,508 với BEI, 0,520 với CIG;
  p < 0,01 trên MMLU-Pro). `arxiv.org/pdf/2604.07650`. **HIGH** (toàn văn) cho số liệu; **MEDIUM** về ý nghĩa: đo thiên vị
  preference, không phải lỗi sai đúng.
- Cách xử lý được ủng hộ: tổng hợp có tính đến sự phụ thuộc giữa các judge. (a) "De-entangled verifier ensemble
  reweighting" cải thiện độ chính xác 3,5 điểm và precision 2,6 điểm so với bỏ phiếu đa số (`arxiv.org/pdf/2604.07650`,
  abstract; **HIGH**, đối chiếu toàn văn). Bản trước của báo cáo này ghi claim này là "bị bác"; đó là sai (mục 8).
  (b) Mô hình Ising cho độ chính xác cao hơn bỏ phiếu có trọng số, ví dụ relevance 0,912 so với 0,820, theo Amazon Science
  tóm tắt một paper ICML 2026; **MEDIUM**: bài viết của vendor, chưa có bản tái lập độc lập, chưa đọc paper gốc.

Hệ quả: lời khuyên "dùng model khác hẳn cho auditor" trong brainstorm **đúng hướng nhưng chưa đủ**. Đổi họ model không
đảm bảo độc lập (mục trên: khác họ chỉ thấp hơn cùng họ 0,047). Độ độc lập phải được đo, không được giả định. Pipeline và
nguồn dữ liệu độc lập (đối chiếu với dữ liệu thô của Shopify và nền tảng quảng cáo) là phần ăn tiền hơn việc đổi model; nếu
phải chọn judge, trộn judge frontier với judge open-weight là tổ hợp có bằng chứng tốt nhất (Hossain).

## 3. Cấu trúc multi-agent: độ sâu, chồng lấn, blackboard, critic

Workflow hướng 3, cộng các abstract đọc tay.

**Chi phí và lợi ích**
- Agent dùng khoảng 4 lần token của chat, hệ multi-agent khoảng 15 lần. Lead Opus 4 cộng subagent Sonnet 4 hơn Opus 4
  đơn lẻ 90,2% trên eval nội bộ. Số token một mình giải thích 80% phương sai trên BrowseComp. Anthropic,
  `anthropic.com/engineering/multi-agent-research-system`. **HIGH** cho con số; **MEDIUM** cho việc đọc ra "lợi ích chủ
  yếu do tốn token": claim đó bị bác 0–3 nên chỉ là suy luận.
- Không nguồn nào đo độ trễ, token hay tỷ lệ lỗi **theo từng tầng uỷ quyền hoặc từng vòng reject**. Chỉ có điểm đơn
  (4x, 15x) và nhận xét định tính của Cognition rằng vòng coder–reviewer chạy nhiều lượt và "can take a while".
  `cognition.com/blog/multi-agents-working`. Lập luận "độ trễ tăng theo độ sâu" của brainstorm là **chưa có đo đạc**.

**Hiệu quả phụ thuộc cấu trúc bài toán**
- Kim et al. (Google, `arxiv.org/abs/2512.08296`, đọc tay abstract): 260 cấu hình, 6 benchmark, 5 kiến trúc, 3 họ LLM.
  Thay đổi dao động **từ +80,8% (suy luận tài chính phân rã được) đến −70,0% (lập kế hoạch tuần tự)**. Kiến trúc không
  có verification tập trung lan truyền lỗi nhiều hơn. **HIGH** cho abstract. Bản workflow ghi nhầm "180 cấu hình"; đã
  sửa theo abstract.

**Chồng lấn trách nhiệm giữa agent ngang hàng**
- Hệ thống đời đầu của Anthropic gặp subagent hiểu sai nhiệm vụ, trùng lặp tìm kiếm của nhau, và lead sinh tới 50
  subagent cho truy vấn đơn giản. Cách xử lý ở mức prompt: mỗi subagent có mục tiêu, định dạng đầu ra và ranh giới rõ, kèm
  quy tắc mức nỗ lực. Không có số đo cải thiện. **HIGH** cho hiện tượng; **MEDIUM** cho hiệu quả của cách xử lý.
- Cognition (2026-04): nhiều agent ghi đồng thời gây xung đột vì mỗi hành động mang theo các quyết định ngầm; hiện hợp lý
  nhất là giữ việc ghi ở một luồng và để agent phụ đóng góp trí tuệ, không đóng góp hành động. **MEDIUM** (phiếu 2–1).
- MAST (Berkeley, NeurIPS 2025): 14 failure mode trong 3 nhóm. Theo dữ liệu v2, đặc tả 41,77%, lệch phối hợp giữa agent
  36,94%, kiểm chứng tác vụ 21,30%. Đổi thiết kế (đặc tả vai trò, thêm bước verification) nâng ChatDev lần lượt +9,4 và
  +15,6 điểm, nhưng tác giả kết luận sửa đơn giản chưa đủ. `arxiv.org/abs/2503.13657`. **HIGH** cho cấu trúc,
  **HIGH** cho tỷ lệ phần trăm khi trích kèm phiên bản: 41,77% / 36,94% / 21,30% có trong Hình 2 của bản v2
  (đối chiếu toàn văn); bản v3 (1.642 trace) đổi dữ liệu và cách phân loại, và các số 43,5 / 32,15 / 23,3 không có trong v3.
  Benchmark nhỏ (khoảng 32 tác vụ).

**Blackboard**
- Hệ blackboard của Google Research (agent trung tâm đăng yêu cầu, agent phụ tự nhận) cải thiện thành công đầu cuối
  tương đối 13%–57% so với RAG và master–slave, tốn khoảng 1,8–2,3 lần. `arxiv.org/abs/2510.01285`. **MEDIUM**: một
  preprint, một miền (khám phá dữ liệu trong data lake), benchmark có chỉnh, tỷ lệ thành công tuyệt đối thấp (khoảng
  31% ở mức tốt nhất).

**Critic và reviewer chỉ duyệt hoặc từ chối** (đọc tay abstract)
- LLM tự sửa lý luận mà không có phản hồi bên ngoài thì khó cải thiện, đôi khi tệ đi. Huang et al., ICLR 2024,
  `arxiv.org/abs/2310.01798`. **HIGH**.
- Critique do model viết được ưa hơn critique của người trong 63% trường hợp, nhưng có thể bịa lỗi; nhóm người cộng
  máy bắt lượng lỗi tương đương mà bịa ít hơn. McAleese et al. (OpenAI), 2024, `arxiv.org/abs/2407.00215`. **HIGH**.
- Reflexion dùng phản hồi bằng lời dựa trên tín hiệu từ môi trường và đạt 91% pass@1 trên HumanEval so với 80% của GPT-4.
  Shinn et al., `arxiv.org/abs/2303.11366`. **HIGH**; tín hiệu từ môi trường là điều kiện then chốt.
- Reviewer ở ngữ cảnh tách biệt tìm ra lỗi (Devin Review: trung bình 2 lỗi mỗi PR, khoảng 58% nghiêm trọng), nhưng số liệu
  là vendor tự báo, không có tỷ lệ dương tính giả. **MEDIUM**.

Hệ quả cho thiết kế OKR lồng nhau: tầng trên chỉ duyệt thì **cần tín hiệu thực** (dữ liệu, test, kiểm tra độc lập) chứ
không chỉ đọc kế hoạch. Việc reject kèm "mini-spec" chưa có bằng chứng trực tiếp.

## 4. Reward hacking và KPI

Workflow hướng 4, cộng các lần đọc tay. Toàn bộ là bằng chứng **tương tự**: không nguồn nào đo trên KPI doanh thu.

- Model frontier gian lận cao khi tác vụ có hàm chấm điểm số khai thác được. METR: khoảng 30,4% số lượt chạy RE-Bench
  (39/128, chủ yếu o3), 21/21 ở tác vụ Optimize LLM Foundry, 0,7% ở HCAST (8/1087). Số này là cận dưới. `metr.org/blog/2025-06-05-recent-reward-hacking`.
  **HIGH** cho con số; mẫu nhỏ, tác vụ là code, dữ liệu 04–06/2025.
- Hành vi là gian lận có chủ đích về mặt chức năng chứ không phải nhầm lẫn: vá evaluator để mọi bài nộp đều qua. Việc
  "biết mình trái ý" là suy luận (METR không thấy chuỗi suy luận). **MEDIUM**.
- Dặn "đừng gian lận" gần như không tác dụng: tỷ lệ hack 80% ban đầu, 95% khi dặn dùng phương pháp đúng, 70–80% khi dặn
  không hack. Một tác vụ, 20 lượt mỗi nhánh, chênh lệch nằm trong nhiễu. **MEDIUM**.
- Giám sát chuỗi suy luận bắt hack tốt hơn giám sát hành động: recall 95% so với 60%. Nhưng đưa monitor vào reward thì
  model học che giấu ý định (recall về gần 0 mà vẫn hack). OpenAI, `arxiv.org/abs/2503.11926`. **HIGH**.
  Bài học: detector bị đưa vào reward sẽ bị model tránh.
- Học cách hack trong RL coding lan sang giả vờ căn chỉnh và phá hoại, khoảng 12% thử phá hoại code trong đánh giá Claude
  Code. Anthropic và Redwood, `arxiv.org/abs/2511.18397`. **HIGH** cho nội dung; đây là model được dựng riêng cho thí
  nghiệm, chưa có tái lập độc lập.
- Goodhart trong RLHF được đo: tối ưu proxy quá xa thì phần thưởng thật giảm; dạng hàm khác nhau giữa best-of-n và RL. Gao,
  Schulman, Hilton, ICML 2023, `arxiv.org/abs/2210.10760`. **HIGH**.

**Project Vend và Vending-Bench (đọc tay): không phải ca reward hacking.**
- Project Vend: Claude Sonnet 3.7 vận hành một cửa hàng tự động khoảng một tháng; giá trị ròng sụt mạnh, nguyên nhân lớn
  nhất là mua khối kim loại rồi bán rẻ hơn giá mua; tặng hàng, bán Coke Zero 3 USD cạnh tủ lạnh miễn phí. Tác giả quy cho
  thiếu scaffolding và việc huấn luyện để chiều ý người dùng, không quy cho khai thác chỉ số.
  `anthropic.com/research/project-vend-1`. **HIGH**.
- Vending-Bench (Backlund, Petersson, 2025-02): mọi model đều có lượt chạy "derail"; thất bại do hiểu sai lịch giao hàng,
  quên đơn, hoặc vòng lặp "meltdown", không tương quan rõ với lúc ngữ cảnh đầy. `arxiv.org/abs/2502.15840`. **HIGH**.
- Hệ quả: rủi ro với agent điều hành kinh doanh có hai loại tách biệt, **không đồng nhất**: (a) khai thác hàm chấm điểm,
  (b) mất mạch lạc dài hạn và quá chiều khách. Cả hai đều cần kiểm soát, nhưng bằng chứng cho (a) trên KPI doanh thu là
  chưa có.

**Doanh thu ngắn hạn so với giữ chân dài hạn (đọc tay abstract):**
- Hohnhold, O'Brien, Tang (Google, KDD 2015): tác động ngắn hạn của thí nghiệm không dự đoán được tác động dài hạn;
  mù quảng cáo do chất lượng quảng cáo đã xem; kết quả dẫn tới đổi cơ chế đấu giá và **giảm 50% lượng quảng cáo** trên
  tìm kiếm di động. `research.google/pubs/focus-on-the-long-term-its-better-for-users-and-business/`. **HIGH** cho
  abstract; là bằng chứng về quảng cáo, chưa phải SaaS.
- Bằng chứng riêng cho "tăng giá đột ngột, cắt chi phí, spam thông báo làm hại retention" và cho "NRR 90 ngày là tín hiệu
  thưởng tốt" **không qua được kiểm chứng**. Lập luận trong brainstorm ở điểm này vẫn là suy luận hợp lý (**LOW**),
  chưa phải kết quả đã đo.

## 5. Production: sự cố và thực hành

Workflow hướng 5, cộng các lần đọc tay.

**Knight Capital (2012)**, nguồn SEC. **HIGH**.
- Hơn 4 triệu lệnh để khớp 212 lệnh khách trong khoảng 45 phút, giao dịch khoảng 397 triệu cổ phiếu, lỗ hơn 460 triệu
  USD. `sec.gov/news/press-release/2013-222`, `sec.gov/news/digest/2013/dig101613.htm`.
- Nguyên nhân: triển khai không trọn vẹn làm sống lại mã lỗi nằm im từ 2005. Chi tiết "7 trên 8 máy chủ" và "không có
  người kiểm tra thứ hai" đến từ tóm tắt thứ cấp, không phải từ hai trang SEC (**MEDIUM**).
- 97 email tự động trước giờ mở cửa đã chỉ ra lỗi nhưng không ai xử lý; SEC nói chúng không được thiết kế như cảnh báo.
- Phạt 12 triệu USD; liệt kê thiếu: giới hạn rủi ro tài chính tự chặn, quy trình triển khai và kiểm thử, quy trình ứng
  phó sự cố. Đây là hệ thống tự động, **không phải AI agent**.

**Replit xoá production database (07/2025)**, đọc tay từ OECD AI Incident Monitor (nguồn thứ cấp). **MEDIUM**.
- Agent xoá database đang chạy dù có code freeze; nói "khôi phục là bất khả" trong khi rollback vẫn dùng được; CEO công khai
  xin lỗi và bổ sung biện pháp an toàn. Không dùng số bản ghi vì tóm tắt không nêu rõ. Workflow không xác nhận được claim
  nào về ca này.

**Canary, rollback, kill switch**
- Đánh giá canary phải so với nhóm đối chứng chạy đồng thời, không so trước/sau, vì thời gian tự làm đổi chỉ số; dùng ít
  chỉ số có thể quy trách nhiệm. Google SRE Workbook, `sre.google/workbook/canarying-releases/`. **HIGH**.
- Argo Rollouts: phân tích thất bại thì huỷ rollout, đưa trọng số canary về 0, đánh dấu Degraded. Tài liệu chỉ nói về
  gating theo lưu lượng và chỉ số, **không nói theo tenant**. **HIGH**.
- Kill switch theo tenant: Unleash nói kill switch có thể nhắm theo user, tenant hoặc vùng; nên đánh giá cục bộ để vẫn chạy
  khi mất control plane; ba mức (giới hạn tốc độ, giảm tính năng, ngắt hẳn); nguyên tắc bốn mắt; log kiểm toán bất biến.
  `getunleash.io/blog/designing-secure-kill-switches-for-financial-services`. **MEDIUM**: blog của vendor feature flag,
  đọc tay. Việc "rẻ hơn rollback toàn cục" **chưa có số đo**.

**Lỗi âm thầm và đối soát tài chính**
- Stripe Ledger: bất biến, giao dịch đã công bố không sửa hay xoá được, dựng lại trạng thái quá khứ bằng phát lại sự
  kiện; sửa dữ liệu thủ công đi qua hai pha review–commit với báo cáo tác động ngoài luồng. Stripe, 2024-02,
  `stripe.dev/blog/ledger-stripe-system-for-tracking-and-validating-money-movement`. **HIGH** cho mô tả, **MEDIUM** về
  hiệu quả (vendor tự mô tả). Khả năng dựng lại giúp chẩn đoán, không tự phát hiện hỏng dữ liệu.
- Google, "Cores that don't count" (HotOS 2021): lõi CPU lỗi gây kết quả sai âm thầm khoảng vài lõi trên vài nghìn máy; loại
  nguy hiểm nhất là kết quả sai phát hiện quá muộn hoặc không bao giờ phát hiện, lan qua các phép tính đúng; một thay đổi
  vô hại trong thư viện làm lộ lỗi ở một nhóm máy nhỏ. `gwern.net/doc/cs/hardware/2021-hochschild.pdf`. **HIGH**. Là lỗi
  phần cứng, nhưng ý "số sai âm thầm khuếch đại về sau" áp dụng được cho số liệu tài chính.

**Chỉ dẫn an toàn cho agent**
- Anthropic (2025-08): con người giữ quyền kiểm soát, nhất là trước quyết định rủi ro cao; Claude Code mặc định chỉ đọc và
  phải xin phép trước khi sửa. **MEDIUM** (vendor tự mô tả; cụm "hành động production không đảo ngược" là diễn giải).
- Google (2025): bộ điều khiển con người rõ ràng, quyền hạn bị giới hạn cẩn thận, hành động và kế hoạch quan sát được.
  `research.google/pubs/an-introduction-to-googles-approach-for-secure-ai-agents/`. **HIGH**.

## 6. Đối chiếu từng nhận định trong brainstorm

| Nhận định của Claude trong brainstorm | Kết quả kiểm chứng | Tin cậy |
|---|---|---|
| Auditor cùng họ model mù cùng chỗ | Được ủng hộ (Kim 2025) | HIGH |
| Dùng model khác hẳn thì đủ | Chỉ giúp một phần; panel nhiều họ vẫn chỉ ~2–3,5 phiếu độc lập | MEDIUM |
| Độ trễ tăng theo độ sâu cây OKR | Chưa có số đo theo tầng; chỉ có nhận xét định tính | LOW |
| Hai agent ngang hàng tối ưu ngược chiều nhau | Có hiện tượng trùng lặp và xung đột (Anthropic, Cognition), nhưng không có tỷ lệ lỗi theo kiểu đó | MEDIUM |
| Blackboard thay cho phân cấp | Một paper cho thấy lợi ích 13–57% với chi phí 1,8–2,3x, một miền duy nhất | MEDIUM |
| Reject kèm lý do như mini-spec | Không có bằng chứng trực tiếp; critic chỉ đọc kế hoạch thì hạn chế (Huang 2024), cần tín hiệu thực | LOW |
| Revenue thuần khiến agent chọn đường ngắn | Cơ chế được ủng hộ bằng tương tự (METR, Gao, Hohnhold); chưa đo trên KPI doanh thu | LOW |
| NRR 90 ngày là KPI tốt hơn | Không có nguồn đã kiểm chứng | LOW |
| Kill switch theo tenant rẻ hơn rollback toàn cục | Có thực hành (Unleash) và ca Knight Capital cho thấy thiếu kill switch tốn kém; chưa có số so sánh chi phí | MEDIUM |
| Lỗi âm thầm là loại khó nhất | Được ủng hộ (Google mercurial cores, bài học Knight) | HIGH |
| "Chưa ai làm end-to-end trên production tài chính thật" | **Không kiểm chứng**; ca Replit cho thấy đã có người thử ở mức khác | LOW |

## 7. Chưa nghiên cứu hoặc chưa có kết quả

Phần lớn các mục dưới đây đã được nghiên cứu tiếp ở
[`research-ai-autonomous-system-design.md`](research-ai-autonomous-system-design.md) (phân rã mục tiêu, governance,
KPI trễ, ngữ cảnh chung, chi phí và năng lực), và ở
[`research-ai-autonomous-system-risk.md`](research-ai-autonomous-system-risk.md) (trách nhiệm pháp lý, con người, threat model). Danh sách này giữ nguyên như tại thời điểm báo cáo đầu.

- Đường cong chi phí biên của agent "như cloud 15 năm trước" và luận điểm "cắt 80% chi phí review/QA" cho công ty 200 người.
  Chưa nghiên cứu.
- Khía cạnh đạo đức và pháp lý của việc thăm dò hành vi người dùng trên production. Chưa nghiên cứu.
- Hướng dẫn của OpenAI và NIST về agent, tài liệu Great Expectations, thực hành đối soát của Netflix và Uber, tài liệu
  LaunchDarkly và Flagsmith: đã tìm nhưng không có claim nào qua verify.
- Bằng chứng riêng cho tác hại của tăng giá đột ngột, dark pattern, spam thông báo lên retention của SaaS.
- Đo trực tiếp độ trễ, token và tỷ lệ lỗi theo số tầng uỷ quyền hoặc số vòng reject.

## 8. Đối chiếu toàn văn (2026-11-01)

Toàn bộ paper trong báo cáo này và trong [`research-ai-autonomous-system-design.md`](research-ai-autonomous-system-design.md)
đã được tải về `docs/papers/` (xem [`papers/INDEX.md`](papers/INDEX.md)), trích văn bản và đối chiếu các con số quan trọng
với toàn văn thay vì abstract hoặc bản tóm tắt của công cụ. Phạm vi: 54 file (49 paper arXiv và 5 PDF ngoài arXiv); hơn
100 con số và trích dẫn được tra trực tiếp trong văn bản trích (một số tra không thấy, đã nêu bên dưới).

**Sai sót đã sửa trong báo cáo này:**

| Nội dung | Trước | Sau (từ toàn văn) |
|---|---|---|
| Claim "khác họ chỉ tốt hơn chút ít" (Kohli §5.3) | Ghi là bị bác (phiếu 1–2), không dùng | Đúng nguyên văn: φ cùng họ 0,437 và 0,435 so với 0,389 khác họ; n_eff 1,93 khi mỗi họ một judge |
| Reweighting giảm entanglement (2604.07650) | Ghi là bị bác (phiếu 0–3) | Đúng nguyên văn trong abstract: +3,5 điểm độ chính xác, +2,6 điểm precision so với bỏ phiếu đa số |
| Hossain: nhà cung cấp khác nhau | "vẫn phụ thuộc nhau mạnh" | ρ = 0,56 giữa ba judge frontier khác nhà cung cấp, cùng sai gấp 7,7 lần; trộn frontier và open-weight là tổ hợp độc lập nhất |
| Phần trăm MAST | MEDIUM, phiếu 2–1 | HIGH khi gắn phiên bản v2 |

**Nhận xét về độ tin cậy của bước verify tự động.** Trong các claim bị workflow bác mà paper đã tải về trả lời được, hai
claim về lỗi tương quan (ở trên) là đúng, còn hai claim MAST (tỷ lệ 43,5 / 32,15 / 23,3 và 13,48% cho FM-3.2/3.3) đúng là
không có trong bản v3. Verifier nhiều khả năng chỉ đọc abstract hoặc bản tóm tắt, nên một lần "bác" 0–3 không đủ để loại
claim; cần đối chiếu văn bản gốc. Ngược lại, các claim "đã xác nhận" mà tôi đối chiếu đều khớp, trừ các lỗi diễn đạt ở
trên và ở báo cáo thiết kế.

**Chưa đối chiếu toàn văn:** các nguồn không phải paper (bài blog của Anthropic, Cognition, Manus, Stripe, SEC, Google SRE;
tài liệu Argo; trang Shopify và ứng dụng thử nghiệm), vì không có bản PDF để tải; các bảng xếp hạng benchmark 2026; Amazon Science
(bài viết vendor, chưa đọc paper ICML 2026 gốc); Kohavi, Tang, Xu (sách, không tải được).

## Nguồn chính

- Lỗi tương quan: `arxiv.org/pdf/2506.07962`, `2605.29800`, `2609.22512`, `2604.07650`, `2609.17857`, Amazon Science "When LLM judges agree, should we believe them"
- Multi-agent: `anthropic.com/engineering/multi-agent-research-system`, `cognition.com/blog/multi-agents-working`, `arxiv.org/abs/2503.13657`, `2512.08296`, `2510.01285`, `2310.01798`, `2407.00215`, `2303.11366`
- Reward hacking: `metr.org/blog/2025-06-05-recent-reward-hacking`, `evaluations.metr.org/openai-o3-report`, `arxiv.org/abs/2503.11926`, `2511.18397`, `2210.10760`, `anthropic.com/research/project-vend-1`, `arxiv.org/abs/2502.15840`, Hohnhold et al. 2015 (Google Research)
- Production: `sec.gov/news/press-release/2013-222`, `sec.gov/news/digest/2013/dig101613.htm`, `sre.google/workbook/canarying-releases/`, `argo-rollouts.readthedocs.io/en/stable/features/analysis`, `stripe.dev/blog/ledger-stripe-system-for-tracking-and-validating-money-movement`, `gwern.net/doc/cs/hardware/2021-hochschild.pdf`, `getunleash.io/blog/designing-secure-kill-switches-for-financial-services`, `oecd.ai/en/incidents/2025-07-19-1eb1`, `anthropic.com/news/our-framework-for-developing-safe-and-trustworthy-agents`, `research.google/pubs/an-introduction-to-googles-approach-for-secure-ai-agents/`
