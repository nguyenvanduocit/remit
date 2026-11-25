# Phản biện 04: luật, quy định và đạo đức

> Thành viên hội đồng: người phản biện pháp lý và đạo đức. Ngày 2026-11-01. Đối tượng: [`01-strategy.md`](01-strategy.md),
> [`02-design.md`](02-design.md), [`03-ledger.md`](03-ledger.md), [`04-launch.md`](04-launch.md) và mục 1 của
> [`research-ai-autonomous-system-risk.md`](../research-ai-autonomous-system-risk.md) (RK).
> **Đây là nghiên cứu phục vụ thiết kế, không phải tư vấn pháp lý.** Bản này ghi điều văn bản nói và câu cần hỏi luật sư.

## 0. Cách đọc

**Mức đọc nguồn** (ghi ở mọi khẳng định pháp lý):
- **[TV-OJ]**: toàn văn chính thức tôi tự mở lần này (Official Journal qua `publications.europa.eu`, Federal Register qua `govinfo.gov`,
  `delcode.delaware.gov`, fiscal note của `leg.colorado.gov`).
- **[TV-DB]**: toàn văn từ cơ sở dữ liệu pháp luật không phải Công báo (`hethongphapluat.com`, bản `.docx` của `luatvietnam.vn`,
  `apolatlegal.com`, `gesetze-im-internet.de`, `law.cornell.edu`). Đúng chữ nhưng chưa đối chiếu Công báo.
- **[TT]**: tóm tắt của hãng luật hoặc báo chí. **[SN]**: chỉ snippet tìm kiếm. **[NHỚ]**: tôi biết văn bản nhưng không mở được
  lần này; cần kiểm lại.

**Nhãn tin cậy** theo `AGENTS.md`: HIGH (đã đọc văn bản), MEDIUM (suy ra hoặc nguồn thứ cấp), LOW (phỏng đoán).

**Giả định:**
- Assumption: công ty là pháp nhân Việt Nam, khách là doanh nghiệp ở Mỹ và EU, một phần là thể nhân kinh doanh (sole trader), và dữ
  liệu khách gồm dữ liệu cá nhân của người mua cuối của họ, vì brief của hội đồng nêu vậy và 03-ledger.md:32 giả định cùng điều đó.
- Assumption: "doanh nghiệp lớn áp dụng blueprint" có thể là một issuer niêm yết ở Mỹ, vì 01-strategy.md:343 định nghĩa công ty lớn có
  bộ phận tài chính đọc chi phí theo loại việc; SOX chỉ áp khi đúng như vậy.
- Assumption: công ty xây hệ thống agent trên model của bên thứ ba và đưa vào dùng dưới tên mình, nên theo cả Luật AI Việt Nam
  (Điều 3(4)) lẫn AI Act (Điều 3(3), Điều 2(1)(c)) nó vừa là provider vừa là deployer.

## 1. Kiểm lại sự kiện pháp lý của sổ và báo cáo rủi ro

| # | Sổ/báo cáo ghi | Văn bản nói | Đã đọc | Kết luận |
|---|---|---|---|---|
| K1 | Omnibus = Reg. (EU) 2026/1744, hiệu lực 27/07/2026 (03-ledger.md:151, MEDIUM) | "REGULATION (EU) 2026/1744 ... of 8 July 2026", OJ L 24.7.2026; Điều 4: "shall enter into force on the third day following that of its publication" | [TV-OJ] | **Đúng, nâng lên HIGH** |
| K2 | Annex III 02/12/2027; Annex I 02/08/2028 | Điều 113(c) mới: "(i) 2 December 2027 as regards ... Annex III; and (ii) 2 August 2028 as regards ... Annex I" | [TV-OJ] | Đúng, HIGH |
| K3 | Điều 50 áp dụng 02/08/2026; "watermark 02/12/2026" (03-ledger.md:151, 337) | Omnibus không dời Điều 50 (điểm (20) chỉ thay khoản 7). Điều 111(4) mới: chỉ provider có hệ thống sinh nội dung "placed on the market before 2 August 2026" được tới 2 December 2026 cho Điều 50(2). | [TV-OJ] | **Sửa:** hạn 02/12/2026 chỉ cho hệ thống có trước 02/08/2026; hệ thống bật ở ngày 0 (sau 02/08/2026) phải đánh dấu máy đọc ngay |
| K4 | (sổ không ghi) | Omnibus thêm cấm Điều 5(1)(ba),(bb) (ảnh khiêu dâm của người thật nhận diện được, CSAM) áp dụng 02/12/2026; viết lại Điều 4: "shall take measures to support the development of AI literacy ... does not require ... any specific level" | [TV-OJ] | Bổ sung. Điều 4 vẫn là nghĩa vụ, nhẹ hơn |
| K5 | EO 11/12/2025 lập AI Litigation Task Force ở DOJ (03-ledger.md:352, MEDIUM) | EO 14365, FR 16/12/2025: Task Force trong 30 ngày; Commerce đánh giá luật bang trong 90 ngày, nêu đích danh Colorado; điều kiện quỹ BEAD; FTC ra policy statement; FCC xét chuẩn liên bang; Mục 8(b) loại trừ an toàn trẻ em; **EO tự nó không preempt luật bang nào** | [TV-OJ] | Đúng, HIGH. Thêm: luật bang vẫn có hiệu lực tới khi toà hoặc Quốc hội nói khác |
| K6 | Colorado SB 26-189 thay SB 24-205, hiệu lực 01/01/2027; luật cũ bị chặn 27/04/2026 (03-ledger.md:351) | Fiscal note bản giới thiệu (04/05/2026): ADMT trong 7 lĩnh vực (giáo dục, việc làm, nhà đất, tài chính và cho vay, bảo hiểm, y tế, dịch vụ công); báo trước khi ADMT "materially influence a consequential decision"; kết quả bất lợi thì giải thích trong 30 ngày, quyền "meaningful human review"; AG thực thi, 60 ngày sửa tới 01/01/2030; "On April 27, 2026, the Attorney General was ordered by U.S. District Court to not initiate enforcement ... X. AI LLC v. Weiser". Ký 14/05/2026 theo Holland & Knight | [TV-OJ] fiscal note; [TT] ngày ký | Đúng, HIGH cho bản giới thiệu. Fiscal note **không có** nghĩa vụ công bố chatbot mà một bài Chambers nêu; đừng trích điều đó |
| K7 | Nghị định 142/2026/NĐ-CP hiệu lực 01/05/2026; rủi ro cao cần quản lý rủi ro, giám sát người (03-ledger.md:365) | Ban hành 30/04/2026, hiệu lực 01/05/2026. Điều 15(2)(c) provider "Thiết kế và duy trì cơ chế giám sát và can thiệp của con người"; Điều 15(5)(c) bên triển khai cũng vậy; Điều 9 định nghĩa rủi ro trung bình; Điều 12 hồ sơ; Điều 14 thông báo "trước khi đưa hệ thống vào sử dụng"; Điều 19 báo cáo sự cố 72 giờ / 05 ngày làm việc, báo cáo chính thức 15 ngày | [TV-DB] | Đúng, HIGH theo văn bản DB. **Thêm Điều 9, 14, 19** (mục 2.3) |
| K8 | QĐ 33/2026/QĐ-TTg: hiệu lực 15/08/2026; mục ngân hàng "AI systems that automatically perform high-value electronic transactions ... without human inspection" (03-ledger.md:366) | Báo chí trong nước trích tên mục: "Hệ thống trí tuệ nhân tạo tự động thực hiện giao dịch điện tử **trong hoạt động ngân hàng**" (VietnamPlus, Tạp chí Công Thương); bản dịch Allen & Gledhill có phần "high-value ... without human inspection, approval or control before execution"; hệ thống có trước 15/08/2026 có hạn 01/09/2027 (tài chính) | [TT] | Ngày đúng (MEDIUM). Cụm "trong hoạt động ngân hàng" làm câu hỏi 1 của sổ hẹp lại: mục này **nhiều khả năng không** áp cho công ty không phải tổ chức tín dụng chi tiền quảng cáo hay hoàn tiền. Chưa đọc toàn văn QĐ 33 |
| K9 | Luật AI: "hệ thống đang chạy có 12 tháng (18 tháng cho tài chính)" (RK §1.3; 01-strategy.md:68) | Điều 35(1): chỉ cho hệ thống "**đã được đưa vào hoạt động trước ngày Luật này có hiệu lực**" (01/03/2026); 18 tháng → 01/09/2027 | [TV-DB] | **Sửa:** hệ thống bật ngày 0 (sau 01/03/2026) **không có** thời gian chuyển tiếp |
| K10 | Điều 7(4) cấm cản trở giám sát người; Điều 29(2) bên triển khai bồi thường (RK §1.3) | Đúng chữ. Điều 29(2) chỉ cho hệ thống **rủi ro cao**, truy đòi provider "nếu có thỏa thuận". Bỏ sót: Điều 4(2) "Bảo đảm duy trì sự kiểm soát và khả năng can thiệp của con người đối với **mọi quyết định và hành vi**"; Điều 12(2)(a) có sự cố nghiêm trọng thì provider "khẩn trương ... khắc phục, tạm dừng hoặc thu hồi hệ thống, đồng thời thông báo" | [TV-DB] | Đúng; thêm Điều 4(2), 9(1)(b), 10(3), 11, 12 |
| K11 | Câu hỏi 2 của sổ: công ty là "nhà cung cấp" hay "bên triển khai" | Điều 3(4): nhà cung cấp đưa hệ thống "vào sử dụng dưới tên, thương hiệu ... của mình, không phụ thuộc hệ thống đó do họ tự phát triển hay được phát triển bởi bên thứ ba"; Điều 3(5) bên triển khai dùng "trong hoạt động nghề nghiệp, thương mại" | [TV-DB] | Văn bản trả lời: **cả hai**. HIGH cho chữ, MEDIUM cho áp dụng |
| K12 | BVDLCN Điều 20(2) 60 ngày; Điều 8(4) 5% doanh thu; Điều 30 phân loại rủi ro; hiệu lực 01/01/2026 | Đúng chữ. 5% chỉ cho vi phạm chuyển dữ liệu xuyên biên giới; vi phạm khác tối đa 3 tỷ đồng (Điều 8(5)); mua bán dữ liệu 10 lần khoản thu (Điều 8(3)) | [TV-DB] | Đúng, HIGH. **Bỏ sót:** Điều 21 (hồ sơ đánh giá tác động **xử lý**, cũng 60 ngày từ lần xử lý đầu), Điều 23 (báo vi phạm 72 giờ), Điều 30(3) ("phân quyền truy cập"), Điều 9(4)(a) (đồng ý "đối với từng mục đích"), Điều 33(2) (chỉ định bộ phận hoặc nhân sự bảo vệ dữ liệu) |
| K13 | Câu hỏi 3: hồ sơ chuyển dữ liệu là khai báo sau hay phê duyệt trước | Điều 20(2) luật: "gửi 01 bản chính ... trong thời gian 60 ngày kể từ ngày đầu tiên chuyển"; Điều 20(1)(b) mọi tổ chức tại Việt Nam chuyển dữ liệu cho bên ở nước ngoài là chuyển xuyên biên giới | [TV-DB]; Nghị định 356 chỉ [SN] | Luật: nộp sau. Nghị định 356 chưa đọc |
| K14 | PLD 2024/2853: phần mềm là sản phẩm; áp cho sản phẩm sau 09/12/2026; chỉ bảo vệ thể nhân (RK §1.2) | Điều 2(1) "placed on the market or put into service after 9 December 2026"; Điều 5(1) "any natural person"; Điều 6(1) chỉ thương tích, tài sản trừ "property used exclusively for professional purposes", dữ liệu "not used for professional purposes" | [TV-OJ] | Đúng, HIGH. **Câu hỏi 6 của sổ được văn bản trả lời phần lớn:** thiệt hại kinh doanh của sole trader do con số sai là thiệt hại kinh tế thuần, ngoài PLD; còn hợp đồng và luật dân sự quốc gia (Điều 2(4)(b)) |
| K15 | Delaware §141(b): giám đốc là thể nhân (02-design.md:426, MEDIUM) | "each of whom shall be a natural person" | [TV-OJ] | Đúng, HIGH. Pháp nhân Việt Nam: Luật Doanh nghiệp 2020 Điều 12(1) người đại diện theo pháp luật "là cá nhân" [TV-DB] |
| K16 | UETA §14 (Minnesota) | Không mở lại. Việt Nam tương đương: Luật Giao dịch điện tử 20/2023/QH15 Điều 14(2)(a), thông điệp "được xem là của người khởi tạo nếu được gửi bởi ... một hệ thống thông tin được thiết lập để hoạt động tự động do người khởi tạo chỉ định" | [TV-DB] | Thêm: lời cam kết của agent ràng buộc công ty cả theo luật Việt Nam |
| K17 | Moffatt v. Air Canada; OLG Hamm (MEDIUM) | CanLII trả 403; bản án OLG Hamm không tìm thấy | — | Giữ MEDIUM; chưa mở |

## 2. Nghĩa vụ chạm vào công ty do agent vận hành, theo nơi

### 2.1 EU (áp vì đầu ra dùng ở EU, khách EU là thể nhân hoặc có nhân viên là thể nhân)

| Lĩnh vực | Nghĩa vụ | Chạm vào blueprint ở đâu | Đọc | Nhãn |
|---|---|---|---|---|
| AI Act, phạm vi | Điều 2(1)(c): provider và deployer ở nước thứ ba "where the output produced by the AI system is used in the Union" | Công ty Việt Nam vẫn thuộc phạm vi | [TV-OJ] | HIGH |
| Minh bạch | Điều 50(1): provider bảo đảm "natural persons concerned are informed that they are interacting with an AI system", trừ khi hiển nhiên; Điều 50(5) chậm nhất "at the time of the first interaction"; áp từ 02/08/2026. Điều 50(1) nói "natural persons", không loại trừ B2B: nhân viên của khách doanh nghiệp là thể nhân | Mọi email, chat, cuộc gọi agent gửi người ngoài | [TV-OJ] | HIGH |
| Đánh dấu nội dung | Điều 50(2) đánh dấu máy đọc cho văn bản, ảnh, âm thanh, video sinh ra (provider); Điều 50(4) deepfake | Agent marketing, design | [TV-OJ] | HIGH |
| Phạt | Điều 99(4)(e),(g): vi phạm Điều 26 hoặc 50 tới 15 triệu EUR hoặc 3% doanh thu toàn cầu | — | [TV-OJ] | HIGH |
| Rủi ro cao (từ 02/12/2027) | Annex III 4(b): AI "to allocate tasks based on individual behaviour ... or to monitor and evaluate the performance and behaviour of persons" trong quan hệ lao động; 5(b) chấm điểm tín dụng thể nhân. Kéo theo Điều 9(8) "testing ... prior to their being placed on the market"; Điều 14(4)(d) người giám sát "disregard, override or reverse the output" **trong từng tình huống**, (e) nút "stop"; Điều 26(2) giao giám sát cho thể nhân có năng lực; Điều 26(7) báo đại diện người lao động trước khi dùng | Agent giao việc, đánh giá hay quản lý người (nhân viên còn lại, contractor ở EU) | [TV-OJ] | HIGH cho chữ; MEDIUM rằng blueprint chạm vào |
| AI literacy | Điều 4 (bản Omnibus): "take measures to support the development of AI literacy" của nhân viên | Owner và người còn lại | [TV-OJ] | HIGH |
| Trách nhiệm sản phẩm | PLD: chỉ thể nhân, chỉ thương tích, tài sản và dữ liệu không dùng cho nghề nghiệp | Hầu như không chạm tới khách B2B (K14) | [TV-OJ] | HIGH |
| GDPR, vai trò | Với dữ liệu người mua cuối của merchant, công ty là processor. Điều 28(3)(a): xử lý "only on documented instructions from the controller"; Điều 28(10): processor tự định mục đích thì bị coi là controller cho việc đó | Đụng thẳng "không giới hạn sáng kiến" khi agent tự dùng dữ liệu đó cho mục đích mới | [NHỚ]; RK §1.4 sub-agent đọc | MEDIUM |
| GDPR, tối thiểu và an toàn | Điều 5(1)(b),(c) mục đích, tối thiểu; Điều 25 bảo vệ từ thiết kế; Điều 32 biện pháp kỹ thuật phù hợp; Điều 33 báo cơ quan trong 72 giờ; Điều 35 DPIA **trước** xử lý | "Mọi agent dùng được mọi connection" (02-design.md:195) | [NHỚ] | MEDIUM |
| GDPR, chuyển dữ liệu | Việt Nam không có adequacy: merchant EU → công ty cần SCC; công ty → nhà cung cấp LLM Mỹ: DPF còn hiệu lực sau Latombe T-553/23 (03/09/2025), đang kháng cáo C-703/25 P | DPA với từng vendor | [SN] cho vụ án | MEDIUM |
| Quyết định tự động | Điều 22: quyết định "solely" tự động có hiệu lực pháp lý hoặc tương tự với thể nhân; CJEU SCHUFA C-634/21 (07/12/2023) coi điểm số quyết định là "decision" | Sole trader: khoá tài khoản, từ chối hoàn tiền, định giá cá nhân; nhân viên: đánh giá, cho nghỉ | [SN] cho vụ án | MEDIUM |
| Lao động | Chỉ thị 98/59/EC Điều 1(1)(a): sa thải tập thể từ 10 người/30 ngày (cơ sở 20–100) hoặc 20 người/90 ngày; Điều 2(1) tham vấn đại diện "with a view to reaching an agreement"; Điều 4(1) hiệu lực không sớm hơn 30 ngày sau khi báo cơ quan. Đức: BetrVG §87(1) Nr. 6 hội đồng lao động đồng quyết việc đưa vào "technischen Einrichtungen, die dazu bestimmt sind, das Verhalten oder die Leistung der Arbeitnehmer zu überwachen" | Bật toàn hệ thống cùng lúc và thay người | [TV-OJ]; [TV-DB] | HIGH cho chữ |

### 2.2 Hoa Kỳ (liên bang và bang chính)

| Lĩnh vực | Nghĩa vụ | Chạm vào đâu | Đọc | Nhãn |
|---|---|---|---|---|
| Chính sách liên bang | EO 14365 không preempt; luật bang vẫn áp (K5) | Không được coi luật bang là đã chết | [TV-OJ] | HIGH |
| Lừa dối | FTC Act §5; "There is no AI exemption" (RK §1.1); Rule về review giả (FTC 08/2024) cấm review "by someone who does not exist, such as AI-generated fake reviews", có phạt dân sự | Agent marketing tự viết testimonial, review; tuyên bố về độ chính xác | Thông cáo FTC [TV-OJ] | HIGH cho nội dung |
| Công bố bot | California B&P §17941 (BOT Act): cấm dùng bot giấu danh tính nhân tạo để thúc đẩy mua bán với người ở California, trừ khi công bố | Agent bán hàng, hỗ trợ | [NHỚ] (leginfo trả 403) | MEDIUM |
| Công bố khi được hỏi | Utah §13-75-103 (sửa bởi SB 226, 2025): phải công bố GenAI khi người tiêu dùng hỏi rõ; safe harbor nếu tự công bố | Như trên | [SN] | MEDIUM |
| ADMT bang | Colorado SB 26-189 từ 01/01/2027 (K6); California CPPA: quy định ADMT có hiệu lực 01/01/2026, tuân thủ ADMT từ 01/01/2027 cho "significant decision" | Chỉ khi agent quyết định về người tiêu dùng hay nhân viên trong lĩnh vực được liệt kê; quyết định B2B thuần nằm ngoài | [TV-OJ] Colorado; [SN] California | HIGH / MEDIUM |
| Hợp đồng | UETA §14: hợp đồng hình thành qua electronic agent dù không ai xem (RK, HIGH cho Minnesota) | Cam kết không trần (02-design.md:199) | RK | HIGH (MN) |
| Lao động | WARN, 29 U.S.C. §2101(a): employer 100+ nhân viên; mass layoff ≥50 người và 33%, hoặc ≥500; báo trước 60 ngày (§2102) | Thay người hàng loạt ở cơ sở Mỹ | [TV-DB] §2101; [NHỚ] 60 ngày | HIGH / MEDIUM |
| Tuyển dụng bằng AI | NYC Local Law 144 (audit thiên lệch AEDT), Illinois HB 3773 | Agent tuyển người | [NHỚ] | LOW |
| Báo cáo tài chính | SOX §404, 15 U.S.C. §7262(a): báo cáo hằng năm nêu "responsibility of management for establishing and maintaining an adequate internal control structure" và đánh giá hiệu lực; §7262(b) kiểm toán viên xác nhận (trừ EGC và issuer không accelerated). SOX §302: CEO và CFO tự ký chứng nhận | Agent chạm vào số liệu đi vào báo cáo tài chính; release không cổng (02-design.md:222) | [TV-DB] §404; [NHỚ] §302 | HIGH / MEDIUM |
| Gọi điện, SMS | TCPA; FCC 02/2024 coi giọng AI là "artificial voice" cần đồng ý trước | Agent gọi hay nhắn tin cho khách Mỹ | [NHỚ] | LOW-MEDIUM |

### 2.3 Việt Nam (nơi pháp nhân đặt trụ sở: mọi nghĩa vụ áp trực tiếp)

| Lĩnh vực | Nghĩa vụ | Chạm vào đâu | Đọc | Nhãn |
|---|---|---|---|---|
| Nguyên tắc AI | Luật 134/2025/QH15 Điều 4(2): AI "không thay thế thẩm quyền và trách nhiệm của con người"; duy trì "sự kiểm soát và khả năng can thiệp của con người đối với mọi quyết định và hành vi" | "Không giới hạn sáng kiến", "không cổng duyệt" | [TV-DB] | HIGH chữ; MEDIUM hệ quả (nguyên tắc, không phải chế tài riêng) |
| Rủi ro trung bình | Điều 9(1)(b) và NĐ 142 Điều 9(1): hệ thống "có khả năng gây nhầm lẫn ... do người sử dụng không nhận biết được chủ thể tương tác là hệ thống trí tuệ nhân tạo". NĐ 142 Điều 9(3)(c),(đ) loại trừ hệ thống không tương tác với công chúng hoặc chỉ xử lý dữ liệu nội bộ | Agent hỗ trợ khách, bán hàng, marketing là rủi ro trung bình; agent nội bộ có thể là rủi ro thấp | [TV-DB] | HIGH chữ; MEDIUM áp dụng |
| Thông báo trước khi dùng | Luật Điều 10(1),(3) và NĐ 142 Điều 12, 14: provider lập hồ sơ phân loại và thông báo Bộ KH&CN qua Cổng một cửa "**trước khi đưa hệ thống vào sử dụng**"; Điều 10(2) phân loại lại khi sửa đổi "làm phát sinh rủi ro mới hoặc rủi ro cao hơn" | Ngày 0; "sửa trong lúc chạy" có thể kích hoạt phân loại lại | [TV-DB] | HIGH |
| Minh bạch | Điều 11(1) người dùng nhận biết khi tương tác với AI; 11(2) đánh dấu máy đọc âm thanh, ảnh, video; 11(3),(4) bên triển khai thông báo, gắn nhãn; Điều 7(2)(b) cấm giả mạo người thật để lừa "có chủ đích và có hệ thống" | Mọi agent nói với người ngoài | [TV-DB] | HIGH |
| Sự cố | Điều 12(2)(a) và NĐ 142 Điều 19: sự cố nghiêm trọng (gồm "thiệt hại đáng kể về tài sản" hay xâm phạm quyền) → khắc phục, tạm dừng hoặc thu hồi; báo sơ bộ 72 giờ (khẩn cấp) hoặc 05 ngày làm việc, báo chính thức 15 ngày; lưu nhật ký | Lớp C0/C1 ở 04-launch.md:101 | [TV-DB] | HIGH |
| Rủi ro cao | Điều 13(3) đánh giá phù hợp là điều kiện để dùng; Điều 14(1)(d) "khả năng giám sát và can thiệp của con người"; QĐ 33 liệt kê 6 lĩnh vực (giáo dục, dân tộc và tôn giáo, y tế, ngân hàng, tố tụng, giao thông) | Chỉ khi công ty hoạt động trong các lĩnh vực đó (K8) | [TV-DB]; [TT] QĐ 33 | HIGH / MEDIUM |
| Dữ liệu cá nhân | Luật 91/2025/QH15: Điều 9(4)(a) đồng ý "đối với từng mục đích", 9(4)(d) im lặng không là đồng ý; Điều 3(2) đúng mục đích; Điều 21 hồ sơ đánh giá tác động xử lý gửi cơ quan trong 60 ngày từ lần xử lý đầu; Điều 20 hồ sơ chuyển xuyên biên giới 60 ngày; Điều 23 báo vi phạm "chậm nhất là 72 giờ"; Điều 30(1) dữ liệu trong AI xử lý "giới hạn trong phạm vi cần thiết", 30(3) hệ thống AI "phải sử dụng phương thức xác thực, định danh phù hợp và **phân quyền truy cập**"; Điều 33(2) chỉ định bộ phận hoặc nhân sự bảo vệ dữ liệu; Điều 38(2),(3) miễn cho doanh nghiệp nhỏ, khởi nghiệp, siêu nhỏ (không áp cho doanh nghiệp lớn) | "Không bí mật", "mọi agent dùng mọi connection" | [TV-DB] | HIGH |
| Lưu trữ tại Việt Nam | Luật An ninh mạng 116/2025/QH15 (hiệu lực 01/07/2026) Điều 25(3): doanh nghiệp cung cấp dịch vụ trên Internet tại Việt Nam có thu thập dữ liệu "của người sử dụng dịch vụ tại Việt Nam" phải lưu trữ dữ liệu này tại Việt Nam theo thời hạn Chính phủ quy định | Chỉ phần người dùng ở Việt Nam (nhân viên, khách Việt Nam) | [TV-DB] | HIGH chữ; MEDIUM phạm vi |
| Hợp đồng điện tử | Luật GDĐT 2023 Điều 14(2)(a) (K16) | Cam kết của agent | [TV-DB] | HIGH |
| Lao động | Bộ luật Lao động 2019 Điều 42(1)(b): "Thay đổi quy trình, công nghệ, máy móc, thiết bị" là thay đổi cơ cấu, công nghệ; 42(3) lập phương án sử dụng lao động (Điều 44), "ưu tiên đào tạo lại"; 42(6) cho thôi việc "chỉ được tiến hành sau khi đã trao đổi ý kiến với tổ chức đại diện người lao động tại cơ sở ... và thông báo trước 30 ngày cho Ủy ban nhân dân cấp tỉnh và cho người lao động"; Điều 47 trợ cấp mất việc mỗi năm 01 tháng lương, ít nhất 02 tháng | Thay nhân viên bằng agent | [TV-DB] | HIGH |
| Pháp nhân | Luật Doanh nghiệp 2020 Điều 12(1): người đại diện theo pháp luật là cá nhân | Ai ký hồ sơ, thông báo, hợp đồng | [TV-DB] | HIGH |

### 2.4 Anh và Singapore

| Nơi | Nghĩa vụ | Đọc | Nhãn |
|---|---|---|---|
| Anh | Không có luật AI riêng; UK GDPR như EU, Điều 22A–22D (Data (Use and Access) Act 2025) thay Điều 22 từ 05/02/2026, cho phép quyết định tự động đáng kể kèm biện pháp bảo vệ | [SN] | MEDIUM |
| Anh | UCTA 1977: điều khoản miễn trừ trong hợp đồng mẫu B2B phải "reasonable" (RK §1.4) | RK | MEDIUM |
| Anh | TULRCA 1992 s.188: tham vấn tập thể khi 20+ người dư thừa trong 90 ngày ở một cơ sở | [NHỚ] (legislation.gov.uk không trả nội dung) | MEDIUM |
| Singapore | PDPA 2012: nghĩa vụ giới hạn chuyển dữ liệu ra nước ngoài | [NHỚ] | MEDIUM |
| Singapore | IMDA "Model AI Governance Framework for Agentic AI" (22/01/2026), tự nguyện: giới hạn trước phạm vi tác động (quyền truy cập công cụ), con người chịu trách nhiệm có thể "override, intercept or review", thông tin cho người dùng cuối | [TT] | MEDIUM. Không bắt buộc nhưng là chuẩn mà khách doanh nghiệp sẽ hỏi |

## 3. Mỗi quyết định của owner: ma sát pháp lý, ai chịu, blueprint phải giữ hoặc thêm gì

Nguyên tắc chung: agent không bao giờ là người chịu trách nhiệm. Người chịu là pháp nhân (provider, deployer, controller, processor,
employer), người đại diện theo pháp luật của nó (Luật Doanh nghiệp Điều 12), và ở issuer Mỹ là CEO và CFO ký §302 (MEDIUM). Luật AI
Điều 4(2) nói thẳng AI "không thay thế thẩm quyền và trách nhiệm của con người" (HIGH chữ).

### 3.1 Bật toàn bộ cùng lúc (big bang)

Luật không cấm bật cùng lúc. Nó đòi một số việc **có trước ngày 0** và một số việc **không thể xảy ra ở ngày 0**.
- **Phải có trước khi dùng (HIGH, văn bản):** hồ sơ phân loại và thông báo Bộ KH&CN cho mọi agent rủi ro trung bình hoặc cao (Luật AI
  Điều 10(3), NĐ 142 Điều 14); DPIA GDPR Điều 35 (MEDIUM, [NHỚ]); công bố tương tác AI từ lần tương tác đầu (AI Act Điều 50(1),(5),
  Luật AI Điều 11(1)); đánh dấu máy đọc nội dung sinh (Điều 50(2); không được hưởng hạn 02/12/2026, K3); DPA và SCC với vendor; không có
  thời gian chuyển tiếp của Luật AI (K9).
- **Nộp trong 60 ngày sau (HIGH):** hồ sơ đánh giá tác động xử lý (BVDLCN Điều 21) và chuyển xuyên biên giới (Điều 20).
- **Không thể ở ngày 0 (HIGH):** cho nhân viên thôi việc. Việt Nam: trao đổi với tổ chức đại diện rồi báo trước 30 ngày (BLLĐ Điều
  42(6)); EU: tham vấn và 30 ngày sau khi báo cơ quan (98/59 Điều 4(1)); Mỹ: 60 ngày nếu tới ngưỡng WARN. Hệ thống bật được ở ngày 0;
  việc thay người thì đi theo lịch luật định. Chính "không có giai đoạn" gặp giới hạn ở đây.
- **Sửa trong lúc chạy:** mỗi sửa đổi làm phát sinh rủi ro mới phải phân loại lại (Luật AI Điều 10(2)); với hệ thống rủi ro cao, "thay
  đổi đáng kể" phải đánh giá phù hợp lại (Điều 13(1)). Sổ release (02-design.md I12) cần một trường "đổi mức rủi ro hay không" do người
  ký. **Thêm:** một danh mục hồ sơ pháp lý ngày 0 (D11 ở 04-launch.md:65 đã có chỗ) với từng văn bản trên, ký bởi người đại diện.

### 3.2 Không giới hạn sáng kiến của agent

Đây là nơi tuân thủ pháp luật (F3) **buộc giới hạn quay lại**. Giới hạn không đặt lên việc agent được nghĩ gì hay làm việc kinh doanh nào,
mà lên **dữ liệu nào cho mục đích nào** và **hành động nào cần một người ký**.
1. **Dữ liệu cá nhân theo mục đích và quyền truy cập.** BVDLCN Điều 30(3) đòi "phân quyền truy cập" trong hệ thống AI xử lý dữ liệu cá
   nhân; Điều 3(2), 9(4)(a), 30(1) giới hạn theo mục đích; GDPR Điều 28(3)(a) bắt processor theo chỉ dẫn của merchant (MEDIUM). Vì vậy
   "mọi agent dùng được mọi connection" (02-design.md:195) và "mọi agent ghi được mọi mục" (02-design.md:130) không đứng được với dữ liệu
   cá nhân. **Phải thêm:** sổ mục đích (purpose register) do người duyệt; `DataClasses` trong `Action` (02-design.md:315) được `Resolve`
   so với sổ đó; agent đọc dữ liệu cá nhân chỉ khi role và mục đích khớp. Đây là một từ chối F3 hợp lệ trong mô hình I1 (02-design.md:50).
2. **Mục đích mới cần cơ sở pháp lý mới.** Agent tự dùng dữ liệu khách cho bộ hồi quy, benchmark, ca công bố (02-design.md:163;
   04-launch.md:178) là mục đích khác; ở EU nó có thể biến công ty thành controller (Điều 28(10), MEDIUM); ở Việt Nam cần đồng ý theo từng
   mục đích (Điều 9(4)(a), HIGH). **Phải thêm:** mục đích mới là một rail người ký.
3. **Liên lạc ra ngoài:** công bố AI ở mọi kênh (mục 4.3); không viết review hay testimonial mang danh người (FTC Rule, HIGH); gọi điện
   hay SMS cho người ở Mỹ cần đồng ý trước (TCPA, LOW-MEDIUM).
4. **Cam kết với khách** ràng buộc công ty (UETA §14; Luật GDĐT Điều 14(2)(a)). Luật không bắt trần; người chịu là pháp nhân. Với
   issuer niêm yết, cam kết về giá và tín dụng chạm doanh thu ghi nhận, tức chạm ICFR (mục 3.5).
5. **Agent quản lý người** (giao việc, đánh giá, cho nghỉ): rủi ro cao theo Annex III 4(b) từ 02/12/2027, cần người có thể "override
   or reverse" từng quyết định (Điều 14(4)(d)); GDPR Điều 22; đồng quyết BetrVG §87(1) Nr. 6 ở Đức. **Phải giữ:** quyết định về việc làm
   của một người cụ thể có người ký.
6. **Điều 4(2) Luật AI** đòi khả năng kiểm soát và can thiệp của người "đối với mọi quyết định và hành vi". Nút dừng cho khả năng can
   thiệp; "kiểm soát" có thể đòi hơn thế. Đây là câu hỏi luật sư số 1 (mục 5).

### 3.3 Không thí nghiệm

Ít ma sát nhất: không A/B trên khách tránh câu hỏi đồng ý và dark pattern. Ba ma sát còn lại (MEDIUM):
- Tuyên bố khách quan về năng lực (độ chính xác, "kết quả đã xác minh") cần bằng chứng có trước khi nói (FTC §5). Không thí nghiệm thì
  chỉ còn bằng chứng quan sát; mọi câu marketing dựa trên scorecard phải ghi đúng giới hạn đó (01-strategy.md:154 đã nói thẳng).
- Hệ thống rủi ro cao phải được test "prior to their being placed on the market" theo chỉ số định trước (AI Act Điều 9(8), HIGH chữ) và
  đánh giá phù hợp trước khi dùng (Luật AI Điều 13(3)). Nếu một chức năng rơi vào rủi ro cao, "không thí nghiệm" phải nhường chỗ.
- "Sửa trong lúc chạy" trên toàn bộ khách là thay đổi không báo trước trên con số khách dựa vào. Không vi phạm luật nào tôi đọc được, nhưng
  hợp đồng với khách phải cho phép điều đó và nói rõ trách nhiệm khi con số đổi.

### 3.4 Mọi thứ mở (cho owner, và ra thế giới theo 04-launch.md §4)

- **Mở cho owner** là quyền của chủ doanh nghiệp, nhưng dữ liệu cá nhân của nhân viên và người mua cuối vẫn chịu giới hạn mục đích và
  phân quyền (BVDLCN Điều 30(3)); owner xem mọi thứ của agent khác với owner đọc mọi dữ liệu cá nhân mà agent đọc. Nếu agent theo dõi
  hiệu suất của người, Đức đòi đồng quyết (HIGH chữ).
- **Mở ra thế giới** (04-launch.md:170–175) va với: (a) dữ liệu cá nhân: mã việc "đã băm" (04-launch.md:172) vẫn có thể là dữ liệu cá nhân
  bút danh hoá theo GDPR (MEDIUM, [NHỚ]); (b) nghĩa vụ bảo mật trong hợp đồng và DPA với khách; (c) thứ tự báo sự cố: cơ quan nhận trong
  72 giờ (BVDLCN Điều 23; GDPR Điều 33; NĐ 142 Điều 19) trước khi công bố postmortem; NĐ 142 Điều 19(3)(c) nói báo cáo sơ bộ "không bị coi
  là sự thừa nhận", còn postmortem công khai thì không có bảo vệ đó; (d) an ninh: công bố brief và mã harness cho kẻ tấn công bản đồ
  injection (GDPR Điều 32, MEDIUM); (e) issuer niêm yết: sự cố trọng yếu có quy tắc công bố riêng (SEC 8-K Item 1.05, [NHỚ], LOW).
- 03-ledger.md:222 hiểu "không bí mật" là trong nội bộ; 04-launch.md §4.2 công bố ra ngoài. Hai bản mâu thuẫn; hội đồng phải chọn một.
- **Phải thêm:** công bố ra ngoài là một rail người ký, sau bước pháp chế (dữ liệu cá nhân, bảo mật hợp đồng, thứ tự báo cơ quan).

### 3.5 Doanh nghiệp lớn niêm yết ở Mỹ: SOX và ICFR

SOX §404 không cấm agent làm kế toán, nhưng ban điều hành phải duy trì và đánh giá kiểm soát nội bộ, và kiểm toán viên kiểm hiệu lực
(HIGH chữ §7262). Release không cổng cho prompt, playbook, model hay judge (02-design.md:222; 04-launch.md:111–113) là thay đổi không
duyệt trong hệ thống có ảnh hưởng tài chính, thiếu tách bạch người làm và người duyệt (MEDIUM, suy luận từ cách ICFR được kiểm). I10
(02-design.md:59) chỉ đưa **mã** tính số vào merge; prompt và cấu hình đổi con số thì không. **Phải giữ:** F4 merge cho mọi release chạm
phạm vi báo cáo tài chính, kể cả prompt và cấu hình; bằng chứng duyệt có thật (93% phê duyệt máy móc, C41, là đúng thứ kiểm toán viên sẽ
thử). Người chịu: CEO, CFO, ủy ban kiểm toán.

## 4. Đạo đức

**Với khách.** Nói thật về việc họ đang làm việc với AI (mục 4.3); không để agent "chiều ý" thành hứa điều công ty không giữ (R3); con
số họ ra quyết định dựa vào là mã có phiên bản (I10, D9) và có đường sửa sai kèm thông báo cho người bị ảnh hưởng. Sole trader là người
thật với rủi ro cá nhân: quyết định bất lợi về họ (khoá, từ chối) có người xem lại được (GDPR Điều 22; Colorado "meaningful human review").

**Với nhân viên bị thay.** Luật đặt mức sàn: phương án sử dụng lao động, ưu tiên đào tạo lại, tham vấn, báo trước, trợ cấp (BLLĐ Điều 42,
47). Đạo đức đòi hơn sàn ở hai điểm có bằng chứng: chức năng bị thay thường quay lại (CBA, Klarna; 04-launch.md §5.4), nên cắt người trước
khi D10 cho thấy chức năng đứng được là chuyển rủi ro sang chính người lao động; và người còn lại không nên thành "lưới" vô hình cho agent
(bài học CBA: trưởng nhóm phải nghe máy). Đề xuất của hội đồng (giả thuyết): quyết định nhân sự đọc D10 và M5 sau ít nhất một chu kỳ đọc
(tuần 8–12), không đọc kế hoạch.

**Với bên thứ ba.** Người mua cuối của merchant chưa bao giờ đồng ý cho một vendor ở Việt Nam đưa dữ liệu của họ qua nhiều agent và nhà
cung cấp LLM; họ chỉ được bảo vệ nếu công ty giữ đúng vai processor. Người nhận email hay cuộc gọi của agent có quyền biết đó là máy.
Người có nội dung web mà agent đọc có thể bị dùng làm đường injection tới chính khách của mình (R5).

**Đồng ý cho xử lý dữ liệu khách trong agent.** EU: dữ liệu người mua cuối dựa trên cơ sở pháp lý của merchant, công ty xử lý theo chỉ dẫn
(Điều 28); dữ liệu của chính khách (sole trader) thường dựa trên hợp đồng (MEDIUM, [NHỚ]). Việt Nam: luật lấy đồng ý làm trung tâm, đồng ý
theo từng mục đích, im lặng không là đồng ý (Điều 9, HIGH). Hệ quả: hợp đồng và thông báo quyền riêng tư phải nêu đích danh việc xử lý bằng
agent và nhà cung cấp LLM ở nước ngoài.

**Minh bạch rằng khách đang nói với AI (luật đã kiểm).** EU Điều 50(1) bắt buộc với thể nhân từ 02/08/2026 (HIGH); Việt Nam Điều 11(1)
bắt buộc, không loại B2B (HIGH); California BOT Act khi thúc đẩy mua bán (MEDIUM); Utah khi được hỏi (MEDIUM); Colorado SB 26-189 không
có nghĩa vụ này trong bản tôi đọc (HIGH cho bản giới thiệu). Quy tắc đơn giản nhất đáp ứng tất cả: **mọi tin nhắn agent gửi người ngoài
nói ngay từ đầu rằng nó là AI**. Quy tắc này nên là mã ở gate, không phải brief.

**Trung thực khi nói về AI.** FTC nhắm tuyên bố lừa dối về AI (Operation AI Comply, RK HIGH); Luật AI Điều 7(5) cấm che giấu thông tin
bắt buộc công khai. 01-strategy.md:88 đã nói "Phần này chưa ai chứng minh có lợi nhuận, kể cả chúng ta"; câu đó phải đi theo mọi tài
liệu ra ngoài. Không dùng "kết quả đã xác minh" (04-launch.md:136) trong marketing khi định nghĩa là của chính công ty.

## 5. Câu hỏi cho luật sư, xếp theo hậu quả

1. **Việt Nam, Luật AI Điều 4(2) và 14:** "duy trì sự kiểm soát và khả năng can thiệp của con người đối với mọi quyết định và hành vi" có
   được đáp ứng bằng nút dừng thủ công, log đầy đủ và quyền đảo ngược sau, khi không có duyệt trước? Điều 4(2) có bị cơ quan xử lý như
   một nghĩa vụ không? Hệ quả: nếu không, "không cổng duyệt" trái luật ở nơi pháp nhân đặt trụ sở.
2. **Việt Nam, phân loại:** agent hỗ trợ khách, bán hàng, marketing có tự động là rủi ro trung bình theo Điều 9(1)(b) và NĐ 142 Điều 9 khi
   đã công bố là AI? Mỗi role là một "hệ thống" phải thông báo riêng, hay toàn bộ là một hệ thống? Sửa brief hay đổi model có là "sửa đổi
   ... làm phát sinh rủi ro mới" (Điều 10(2))?
3. **GDPR và BVDLCN, vai trò:** với dữ liệu người mua cuối, khi agent tự chọn việc, công ty còn là processor không (Điều 28(10))? Mức
   phân quyền truy cập nào thoả BVDLCN Điều 30(3) và GDPR Điều 25, 32?
4. **Lao động:** thay nhân viên bằng agent ở Việt Nam có phải thủ tục Điều 42 và 44 BLLĐ cho từng đợt không, và cơ sở chưa có tổ chức đại
   diện thì trao đổi với ai? Ở EU và Mỹ (nếu có nhân viên): ngưỡng 98/59, WARN, TULRCA; ở Đức: §87(1) Nr. 6 và §111 BetrVG.
5. **Sự cố:** ngưỡng "thiệt hại đáng kể về tài sản" (NĐ 142 Điều 19(1)(b)) với một con số sai gửi tới nhiều merchant là gì? Thứ tự báo
   cơ quan Việt Nam, cơ quan EU, khách và công bố postmortem ra sao?
6. **EU, Điều 50:** dạng công bố đủ cho email và chat B2B; email do agent soạn mà người ký gửi có thuộc Điều 50(1) không? Công ty có phải
   chỉ định đại diện EU (GDPR Điều 27) khi là processor không?
7. **SOX (nếu issuer Mỹ áp dụng):** release prompt, playbook, model chạm số liệu tài chính có phải là thay đổi chương trình trong phạm vi
   ITGC không; bằng chứng duyệt nào kiểm toán viên chấp nhận?
8. **Hợp đồng với khách:** điều khoản nào giới hạn trách nhiệm cho con số do agent tạo và cho cam kết của agent, có hiệu lực ở California,
   New York, Đức, Anh (UCTA) và Việt Nam? Có cách hợp lệ rút cam kết sai của agent (Luật GDĐT Điều 14(3) về lỗi nhập qua hệ thống tự động)?
9. **QĐ 33:** "giao dịch điện tử trong hoạt động ngân hàng" có chạm tới công ty không phải tổ chức tín dụng tự động thanh toán không? Xin
   toàn văn QĐ 33 trên Công báo.
10. **Chuyển dữ liệu:** Nghị định 356 có đòi gì thêm Điều 20 (mẫu, thời hạn, phê duyệt)? Một hồ sơ cho mọi nhà cung cấp LLM hay mỗi bên một
    hồ sơ? Luật An ninh mạng 2025 Điều 25(3) có chạm tới công ty có khách chủ yếu ở nước ngoài không?
11. **Công bố ra ngoài:** công bố brief, playbook, bảng mỗi lần chạy và postmortem có vi phạm DPA, NDA với khách, hay tạo thừa nhận trách
    nhiệm không?
12. **Bảo hiểm:** hợp đồng hiện có loại trừ thiệt hại do AI sinh không (giữ từ 03-ledger.md:388); Luật AI Điều 14(5) khuyến khích bảo hiểm
    trách nhiệm cho hệ thống rủi ro cao.

## 6. Câu trong bản nháp sai hoặc nguy hiểm nếu công bố

| Chỗ | Câu | Vấn đề | Sửa |
|---|---|---|---|
| 01-strategy.md:68–69 | "hệ thống đang chạy tuân thủ trước 01/03/2027 (18 tháng cho tài chính)" | Điều 35 chỉ cho hệ thống có trước 01/03/2026; hệ thống bật ngày 0 không có hạn đó (K9) | "Hệ thống bật sau 01/03/2026 tuân thủ ngay khi đưa vào sử dụng" |
| 01-strategy.md:189 | "agent không bị giới hạn phạm vi" | Dữ liệu cá nhân bị giới hạn theo mục đích và phân quyền (BVDLCN Điều 30(3)) | "... trừ giới hạn dữ liệu theo mục đích mà luật đòi" |
| 01-strategy.md:283–284 | "Điều 7(4) ... tức là nút dừng của owner cũng là nghĩa vụ pháp lý" | Điều 7(4) cấm cản trở cơ chế giám sát; nghĩa vụ có cơ chế nằm ở Điều 4(2), 14; nút dừng có thể chưa đủ | Ghi đúng điều và câu hỏi 1 |
| 02-design.md:130 | "mọi agent ghi được mọi mục" | Như trên với mục chứa dữ liệu cá nhân | Mục có `DataClasses` được phân quyền |
| 02-design.md:195 | "Giới hạn phạm vi: mọi agent dùng được mọi connection" (cột "Không là") | Trái BVDLCN Điều 30(3) với connection chứa dữ liệu cá nhân; nguy cơ processor thành controller | Connection có dữ liệu cá nhân gắn với role và mục đích |
| 02-design.md:196 | hồ sơ chuyển dữ liệu "là việc có hạn của một role" | Hồ sơ là tự kê khai, "tự chịu trách nhiệm trước pháp luật" (NĐ 142 Điều 14(1)); người đại diện là cá nhân (LDN Điều 12) | Agent soạn; người đại diện ký và nộp, như rail merge |
| 02-design.md:163 | "Lặp eval trên tác vụ ghi lại không phải thí nghiệm trên khách" | Đúng về thí nghiệm, nhưng là xử lý dữ liệu khách cho mục đích mới | Thêm: cần cơ sở pháp lý cho mục đích test |
| 02-design.md:199 | Cam kết với khách "không bị giới hạn bằng mã" | Ràng buộc pháp nhân (Luật GDĐT Điều 14(2)(a)); với issuer niêm yết chạm ICFR | Giữ quyết định, nhưng ghi ai chịu và câu hỏi 8 |
| 02-design.md:364 | "Không có thí nghiệm thì không có rủi ro pháp lý và đạo đức của việc thử trên khách" | Sửa trong lúc chạy vẫn đổi trải nghiệm khách không báo trước; rủi ro cao vẫn phải test trước (Điều 9(8)) | "giảm rủi ro của A/B; không bỏ nghĩa vụ test trước với hệ thống rủi ro cao" |
| 02-design.md:426–427 | Giám đốc phải là thể nhân "(Delaware DGCL §141(b))" | Đúng cho Delaware; pháp nhân là công ty Việt Nam | Thêm LDN 2020 Điều 12(1) |
| 04-launch.md:101 | C0: "Owner bấm P1 nếu muốn" | Luật AI Điều 12(2)(a) đòi khắc phục, tạm dừng hoặc thu hồi và thông báo; BVDLCN Điều 23 và NĐ 142 Điều 19 đặt hạn 72 giờ | C0 kéo theo báo cơ quan đúng hạn; dừng là quyết định của owner, báo cáo thì không |
| 04-launch.md:170–174 | Công bố brief, harness, bảng mỗi lần chạy, postmortem | Mục 3.4 | Công bố là rail người ký sau pháp chế |
| 04-launch.md:178–179 | Ca hồi quy từ việc thật "công bố dưới dạng bản sao tổng hợp" | Mục đích mới; bảo mật hợp đồng; ẩn danh chưa có bằng chứng | Cần cơ sở pháp lý và kiểm ẩn danh trước |
| 04-launch.md:219 | "NIST AI RMF MANAGE 2.4 yêu cầu" | NIST AI RMF là khung tự nguyện | "khuyến nghị" |
| 04-launch.md:229–231 | Omnibus "chưa đọc Official Journal" | Nay đã đọc (K1–K3) | Ghi Reg. (EU) 2026/1744, OJ 24/07/2026, HIGH |
| 04-launch.md:236 | Tham vấn đại diện người lao động "(bài học CBA)" | Là nghĩa vụ luật ở Việt Nam, EU, Đức, Mỹ (mục 2) | Ghi là nghĩa vụ, kèm điều luật |
| 03-ledger.md:151, 337 | "watermark 02/12/2026" | Chỉ cho hệ thống có trước 02/08/2026 (K3) | Sửa như K3 |
| 03-ledger.md:222 | "'Không bí mật' là trong nội bộ" | Mâu thuẫn với 04-launch.md §4.2 | Hội đồng chọn một |

## 7. Điều đứng vững

- F2 (credential ngoài ngữ cảnh agent) phù hợp mọi văn bản tôi đọc; nó hỗ trợ GDPR Điều 32 và BVDLCN Điều 30(3) về xác thực.
- F1 đặt ở gateway, ngoài tầm agent (02-design.md I5; 04-launch.md D1) đúng với AI Act Điều 14(4)(e) ("stop" button) và Luật AI Điều 7(4);
  nó là điều kiện cần, chưa chắc đủ (câu hỏi 1).
- Log do nền tảng ghi, chỉ thêm (I4, D4) đáp ứng nghĩa vụ lưu nhật ký sự cố (NĐ 142 Điều 19(4)) và cung cấp hồ sơ khi thanh tra (Luật AI
  Điều 28(3)).
- Số khách dựa vào do mã có phiên bản tính (I10, D9) là kiểm soát đúng hướng cho cả trách nhiệm hợp đồng và ICFR.
- `LegalPolicy` trong `Resolve` (02-design.md:320) là chỗ đúng để đặt giới hạn mục đích và quyền truy cập dữ liệu mà mục 3.2 đòi: thiết
  kế đã có khe cho nó, chỉ thiếu nội dung.
- Không A/B trên khách (quyết định 3) loại bỏ một nhóm rủi ro đồng ý và dark pattern (DSA Điều 25, FTC).
- 01-strategy.md:88 và 154 nói thẳng rằng lợi nhuận chưa được chứng minh và ROI chỉ là tương quan: đó là đúng chuẩn trung thực FTC đòi.
- Ledger đúng ở các chữ nó trích từ Luật 91/2025 và Luật 134/2025 (K10, K12); chỉ thiếu điều.

## Kết luận cho báo cáo cuối

Một doanh nghiệp lớn **có thể** áp dụng blueprint hợp pháp nếu blueprint giữ năm điều mà luật đòi: (1) giới hạn dữ liệu cá nhân theo
mục đích và quyền truy cập, thực thi ở gate; (2) các hồ sơ, thông báo, công bố ra ngoài và báo cáo sự cố là rail do một cá nhân ký; (3)
công bố "đây là AI" bằng mã ở mọi kênh ra ngoài; (4) thay người theo thủ tục và lịch luật lao động, không ở ngày 0; (5) với issuer niêm yết,
merge cho mọi release chạm số liệu tài chính. Câu trả lời cho câu hỏi 1 (Điều 4(2) Luật AI) có thể buộc thêm giới hạn thứ sáu. HIGH rằng
năm điều trên có trong văn bản; MEDIUM về cách cơ quan áp dụng.

## Nguồn đã mở lần này

- EU: `publications.europa.eu/resource/celex/32026R1744` (Omnibus, OJ 24.7.2026); `publications.europa.eu/resource/cellar/dc8116a1-3fe6-11ef-865a-01aa75ed71a1.0006.03/DOC_1` (AI Act); `publications.europa.eu/resource/celex/32024L2853` (PLD); `publications.europa.eu/resource/celex/31998L0059` (sa thải tập thể); `gesetze-im-internet.de/betrvg/__87.html`.
- Mỹ: `govinfo.gov/content/pkg/FR-2025-12-16/html/2025-23092.htm` (EO 14365); `leg.colorado.gov/bill_files/115717/download` (fiscal note SB 26-189); `hklaw.com/en/insights/publications/2026/05/colorado-governor-signs-sb-189`; `law.cornell.edu/uscode/text/29/2101`, `.../15/7262`; `delcode.delaware.gov/title8/c001/sc04/index.html`; `ftc.gov` (thông cáo 08/2024 về review giả).
- Việt Nam: `static3.luatvietnam.vn/.../luat-134-2025-qh15_2512152052.docx` (Luật AI); `hethongphapluat.com` (NĐ 142 Điều 9, 12, 14, 15, 19; Luật 91/2025 Điều 1, 3, 8, 9, 20, 21, 23, 30, 33, 38; BLLĐ Điều 42, 47; Luật GDĐT 2023 Điều 14, 18, 36; LDN 2020 Điều 12; NĐ 53/2022 Điều 26); `apolatlegal.com/wp-content/uploads/2026/08/Luat-116-2025-QH15.docx` (Luật An ninh mạng 2025); `vietnamplus.vn`, `tapchicongthuong.vn`, `allenandgledhill.com` (QĐ 33, tóm tắt).
- Không mở được: EUR-Lex HTML (chặn bot; dùng Publications Office thay), toàn văn GDPR (404), `leginfo.legislature.ca.gov` (403), CanLII (403), `legislation.gov.uk` (không trả nội dung), `le.utah.gov`, eCFR (chuyển hướng), toàn văn QĐ 33 và Nghị định 356, bản án OLG Hamm, mẫu ISO bảo hiểm.
