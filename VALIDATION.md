# Validação local — 24 de outubro de 2026

Compilação `release`, Windows x64, neste computador. Estes números são medições locais, não uma previsão para o disco inteiro nem uma comparação com TreeSize.

## Abertura como administrador — correção de 25 de outubro de 2026

- A janela original fecha automaticamente apenas após confirmação da nova instância elevada, enviada depois de desenhar um primeiro frame da interface. A nova janela recebe o mesmo caminho para reanalisar.
- A confirmação utiliza um evento exclusivo para cada tentativa e é cruzada com o processo criado pelo Windows. Uma chamada de abertura bem-sucedida, por si só, não fecha a janela original.
- Cancelamento do pedido de administrador, falha de abertura, saída prematura ou ausência de confirmação mantêm a janela original. A espera corre no worker, com timeout após a criação do processo, sem bloquear a interface.
- **30 testes aprovados, 2 fixtures de integração ignoradas por omissão.** Os testes novos cobrem confirmação com processo vivo, saída sem confirmação, timeout sem terminar o processo e recusa de confirmação por um processo sem privilégios. Os subprocessos de teste não pedem elevação nem acionam o UAC. Clippy sem avisos e formatação aprovados.
- A aprovação real do UAC não foi automatizada.
- Executável portátil atualizado e verificado por hash contra a compilação `release`. Como a janela já aberta estava em modo administrador, foi conservada; a versão anterior em execução ficou guardada em `target/` durante a substituição do ficheiro portátil.

## Atualização 0.4 — 25 de outubro de 2026

- **26 testes aprovados**, mais um teste de integração de Reciclagem ignorado por omissão. As novas regras cobrem nomes de pasta enganadores, manifestos e marcadores de compilação, ficheiros inesperados em pastas geradas, sistema, cloud, metadados parciais, datas desconhecidas/futuras, não sobreposição e um limite de 200 resultados com 10 000 candidatos. Formatação e Clippy sem avisos aprovados.
- Os dois testes novos de junctions recusam um redirecionamento para dentro da mesma raiz e a substituição posterior de um ancestral. Nenhum deles recicla ficheiros; usam apenas fixtures geradas para o teste.
- UI verificado na janela nativa: navegação lateral, resumo, recomendações, tipos de ficheiro e o fluxo **Rever na árvore**. As recomendações não executam nem eliminam o item.
- No próprio projeto, o motor identificou `target/` pelos marcadores Cargo e pelo manifesto, com 2 505 303 074 bytes observados na medição inicial da versão. A sugestão explica que é necessário poder recompilar, fechar builds e confirmar que não foram guardados ficheiros manuais. Não foi efetuada limpeza.
- Análise local de `C:\`, em modo normal, com 16 workers: **2 282 184 ficheiros**, **411 106 pastas**, **941 239 867 991 bytes lógicos medidos**, **25,40 s** de scanner e **25,74 s** para o processo completo com recomendações e leitura do JSON no PowerShell. Registou 478 acessos negados e 1 476 ligações ignoradas. A cache e mudanças no disco tornam estas medições variáveis; a cobertura é parcial.
- Foram encontradas **53 sugestões**: 3 artefactos Cargo recriáveis sujeitos a revisão (14 460 821 485 bytes), 2 localizações a gerir pelo Windows e 48 ficheiros pessoais para inspeção (38 822 803 763 bytes). Os valores não são uma promessa de espaço físico recuperável e não houve dupla contagem nos totais.
- A análise de recomendações corre separadamente da interface, com um relatório vinculado ao mesmo `Arc<Scan>`; um relatório tardio de uma análise anterior não é aplicado.
- Uma verificação independente dos 53 caminhos recomendados confirmou zero sobreposições entre ancestrais e descendentes. Todas as ações dos cartões são de inspeção ou abertura de ferramentas do Windows; nenhuma executa limpeza.
- O executável portátil final corresponde à compilação `release`. A CLI final confirmou os 15 ficheiros e 210 265 bytes de `src/` com uma enumeração independente, sem recomendações artificiais para a pasta de fontes.
- Na verificação final da janela, a reanálise de `C:\` demorou 12,34 s e voltou a apresentar 53 sugestões e 13,47 GiB lógicos em artefactos recriáveis sujeitos a revisão. A diferença face à execução anterior ilustra o efeito da cache/estado do disco. A app ficou aberta na página Recomendações; nenhuma limpeza foi executada.

## Atualização 0.3 — 25 de outubro de 2026

- **15 testes automáticos aprovados**, mais o teste nativo de Reciclagem executado separadamente. Clippy sem avisos e formatação aprovados. O teste de Reciclagem fica ignorado por omissão para que `cargo test` não mova ficheiros sem intenção explícita.
- O teste nativo enviou apenas uma pasta descartável gerada em `target/`, com um ficheiro de teste, para a Reciclagem. Foram também testadas as recusas da raiz da análise, de caminhos exteriores e de ficheiros alterados desde a preparação.
- A confirmação visual de uma pasta exige escrever o nome antes de ativar o botão. Na verificação da interface foi usado **Cancelar**, e a pasta de preview permaneceu intacta. A pasta de preview foi depois removida sem recursão, com o caminho e conteúdo verificados.
- Menu de contexto, seleção por botão direito, ordenação clicando na área do cabeçalho e janela nativa de propriedades verificados. A janela de propriedades foi fechada sem alterações.
- A CLI em `src/` devolveu **13 ficheiros**, **138 235 bytes**, zero erros e zero entradas incompletas; totais confirmados por uma enumeração independente.
- A versão final analisou `C:\` em modo normal: **2 281 328 ficheiros**, **410 985 pastas**, **940 869 640 838 bytes lógicos medidos**, em **10,14 s**. Registou **477 pastas sem permissão**, **zero outros erros** e **1 476 ligações ignoradas**. São resultados parciais, explicitamente assinalados na interface e nos ramos afetados.
- Janela **Cobertura e permissões** verificada com os erros reais de `C:\`, incluindo `PerfLogs`, `System Volume Information` e `ProgramData`. A explicação indica que administrador é opcional, pode aumentar a cobertura e não garante acesso a todas as pastas.
- O botão de elevação não foi acionado automaticamente; não foram alteradas permissões do Windows. A construção dos argumentos com espaços e barras finais tem um teste dedicado.
- O executável portátil corresponde à compilação `release` da versão 0.3 e ficou aberto na análise do disco C.

## Atualização 0.2 — 25 de outubro de 2026

- **12 testes aprovados**, incluindo conversão de FILETIME, datas nativas e agregação de datas por ramo, expansão/recolha da árvore, invalidação da ordenação em cache, pesquisa com antepassados e filtros por extensão. Formatação e Clippy sem avisos também passaram.
- Árvore expansível, atualização de propriedades/mapa ao selecionar uma pasta, ordenação por nome e vista por extensões verificadas na janela nativa. O duplo clique em `.exe` abriu os 102 ficheiros correspondentes da análise de teste.
- CSV exportado pelo diálogo nativo e relido independentemente: **4 754 nós**, incluindo **4 084 ficheiros**, **1 771 391 798 bytes**. A soma dos ficheiros coincidiu com a raiz. As nove colunas estavam presentes, incluindo datas Unix e atributos; todos os ficheiros tinham data de modificação.
- A CLI portátil analisou os **10 ficheiros** de `src/`, totalizando **98 323 bytes**. Uma enumeração independente confirmou contagem, tamanho e data de modificação de uma entrada.
- Nova medição na pasta de projetos, com os detalhes ativos e 16 workers: **235 213 ficheiros**, **25 013 subpastas**, **19 074 732 179 bytes**, em **1,417 s**; **zero erros** e **4 reparse points ignorados**. A cache e as alterações na pasta impedem uma comparação direta com a medição inicial abaixo.
- Executáveis finais da versão 0.2 em `dist/SpaceFinder.exe` e `dist/SpaceFinder-CLI.exe`.
- Corrigida a captura de cliques pelos textos das células. O clique direto no nome seleciona agora a linha e atualiza de imediato o painel de propriedades e o mapa; expansão por duplo clique também verificada. Os 12 testes e o Clippy voltaram a passar após a correção.

As secções seguintes conservam as medições da versão inicial.

## Pasta de projetos

Uma análise de `C:\Users\<utilizador>\Documents\Projs` (caminho anonimizado), com 16 workers:

| Métrica | Resultado |
| --- | ---: |
| Ficheiros | 234 544 |
| Subpastas | 24 947 |
| Bytes lógicos | 18 869 397 446 |
| Tempo do scanner, incluindo agregação | 3,872 s |
| Erros de leitura | 0 |
| Reparse points ignorados | 4 |

O disco, a cache do sistema e o antivírus influenciam estes resultados. A pasta pode mudar enquanto a análise corre.

## Verificação independente de totais

Na cache de fontes de Cargo (`C:\Users\<utilizador>\.cargo\registry\src`, caminho anonimizado), análises com 1, 4 e 16 workers produziram os mesmos **6 852 ficheiros**, **1 664 subpastas** e **158 349 421 bytes**. Uma enumeração independente com `Get-ChildItem -Recurse -File -Force` e `Measure-Object Length -Sum` confirmou exatamente a quantidade de ficheiros e os bytes.

Tempos das três execuções sequenciais: 0,167 s, 0,089 s e 0,070 s. A ordem e o aquecimento da cache impedem atribuir a diferença apenas ao paralelismo.

## Verificação funcional

- `cargo test --all-targets`: 7 testes aprovados, cobrindo totais, Unicode, ocultos, pastas vazias, caminhos inválidos, ficheiros como raiz, lotes maiores que 1 024 entradas, cancelamento, backpressure, caminhos com mais de 260 caracteres e ciclos em junctions.
- `cargo clippy --all-targets -- -D warnings`: aprovado.
- `cargo fmt --check`: aprovado.
- Build otimizada e executáveis portáteis gerados em `dist/`.
- Interface aberta e inspecionada no Windows; vista dos maiores ficheiros verificada.
- Exportação feita pelo diálogo nativo: o CSV continha 4 022 nós, incluindo 3 417 ficheiros. A soma independente dos tamanhos dos ficheiros (1 573 604 700 bytes) coincidia com o total da raiz exportada.

