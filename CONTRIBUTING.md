# Contribuindo com o Kirei

Obrigado pelo interesse. Este guia explica como preparar o ambiente, o que esperamos de um pull request e como relatar problemas. Issues e PRs podem ser escritos em português, inglês ou espanhol.

## Preparar o ambiente

1. Instale as Command Line Tools do Xcode: `xcode-select --install`. O Xcode completo não é necessário.
2. Clone o repositório e compile:

   ```bash
   git clone https://github.com/shiddo-tech/kirei.git
   cd kirei
   scripts/build.sh --install
   ```

3. Se você recompila com frequência, crie o certificado local **Kirei Local Signing**. Sem ele, o macOS pede a permissão de Gravação de Tela de novo a cada build. O passo a passo está no README, em [Assinatura local e permissões](README.md#assinatura-local-e-permissões).

Você não precisa de conta de desenvolvedor da Apple para contribuir.

## Antes de abrir um pull request

- Para mudanças grandes (funcionalidade nova, refatoração, troca de comportamento), abra uma issue antes. Assim combinamos a abordagem e ninguém perde trabalho.
- Mantenha cada PR focado em um assunto.
- Confirme que `swift build` compila sem erros.
- Teste no `.app` gerado pelo `scripts/build.sh`, e não no binário solto em `.build/`. O binário solto não tem Info.plist, ícone nem traduções.
- Se mexer no renderizador ou na captura com rolagem, rode os autotestes descritos no README, em [Autotestes](README.md#autotestes).
- Em mudanças visuais, anexe capturas de antes e depois no PR.

## Estilo de código

- Siga o estilo dos arquivos ao redor: Swift e SwiftUI, sem dependências externas.
- Escreva comentários só quando o código sozinho não explica a regra, como um contorno de bug do macOS ou uma restrição externa.
- Todo texto visível para o usuário passa por `L("English text")`. A chave é o texto em inglês.

## Traduções

O app está em inglês, português (Brasil) e espanhol. Se você incluir ou alterar um texto, atualize os três arquivos em `Localization/<idioma>.lproj/Localizable.strings`. Se não souber traduzir, deixe a chave em inglês e avise no PR.

Para incluir um idioma novo, siga os passos em [Idiomas](README.md#idiomas), no README.

## Privacidade

O Kirei funciona 100% no Mac, sem conta, servidor ou envio de dados. Não aceitamos PRs que adicionem telemetria, analytics ou chamadas de rede sem que o usuário peça.

## Relatar um bug

Use o template de bug em [Issues](https://github.com/shiddo-tech/kirei/issues/new/choose) e informe:

- versão do Kirei (em **Ajustes**) e do macOS;
- passos para reproduzir o problema;
- o resultado esperado e o que aconteceu de fato.

Se o app travou ou congelou, anexe os relatórios da pasta `~/Library/Logs/Kirei`. Antes de anexar capturas, confira se elas não mostram dados seus.

## Licença

Ao contribuir, você concorda que sua contribuição será distribuída sob a [licença MIT](LICENSE) do projeto.
