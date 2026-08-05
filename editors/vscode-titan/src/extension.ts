import * as vscode from 'vscode';
import { LanguageClient, LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClient | undefined;
function setting(name: string): string | boolean { return vscode.workspace.getConfiguration('titan').get(name)!; }
function target(): string | undefined {
    const editor = vscode.window.activeTextEditor;
    if (editor?.document.languageId === 'titan') return editor.document.uri.fsPath;
    return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}
async function runTask(label: string, subcommand: string): Promise<void> {
    const input = target();
    if (!input) return void vscode.window.showErrorMessage('Abre un archivo .titan o una carpeta de proyecto TITAN.');
    const args = [subcommand, input];
    if ((subcommand === 'run' || subcommand === 'test') && setting('run.sandbox') === true) args.push('--sandbox');
    const execution = new vscode.ProcessExecution(String(setting('compiler.path')), args, { cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath });
    const task = new vscode.Task({ type: 'titan', task: label }, vscode.TaskScope.Workspace, label, 'TITAN', execution, ['$gcc']);
    await vscode.tasks.executeTask(task);
}
class TitanDebugAdapterFactory implements vscode.DebugAdapterDescriptorFactory {
    createDebugAdapterDescriptor(): vscode.ProviderResult<vscode.DebugAdapterDescriptor> { return new vscode.DebugAdapterExecutable(String(setting('dap.path'))); }
}
export async function activate(context: vscode.ExtensionContext): Promise<void> {
    const serverOptions: ServerOptions = { command: String(setting('lsp.path')), args: [], options: { cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath } };
    const clientOptions: LanguageClientOptions = { documentSelector: [{ scheme: 'file', language: 'titan' }, { scheme: 'untitled', language: 'titan' }], synchronize: { fileEvents: vscode.workspace.createFileSystemWatcher('**/*.titan') }, outputChannelName: 'TITAN Language Server' };
    client = new LanguageClient('titan-lsp', 'TITAN Language Server', serverOptions, clientOptions);
    void client.start();
    context.subscriptions.push(vscode.debug.registerDebugAdapterDescriptorFactory('titan', new TitanDebugAdapterFactory()));
    for (const [command, label, action] of [['titan.run', 'Run TITAN', 'run'], ['titan.check', 'Check TITAN', 'check'], ['titan.build', 'Build TITAN bytecode', 'build'], ['titan.wasm', 'Build TITAN WebAssembly', 'wasm'], ['titan.test', 'Test TITAN project', 'test']] as const) context.subscriptions.push(vscode.commands.registerCommand(command, () => runTask(label, action)));
    context.subscriptions.push(vscode.commands.registerCommand('titan.debug', async () => {
        const program = target(); if (!program) return void vscode.window.showErrorMessage('Abre un archivo .titan o una carpeta de proyecto TITAN.');
        await vscode.debug.startDebugging(vscode.workspace.workspaceFolders?.[0], { type: 'titan', request: 'launch', name: 'Debug TITAN', program, sandbox: setting('run.sandbox') === true });
    }));
    context.subscriptions.push(vscode.commands.registerCommand('titan.newProject', async () => {
        const folder = await vscode.window.showOpenDialog({ canSelectFiles: false, canSelectFolders: true, canSelectMany: false, openLabel: 'Seleccionar carpeta padre' }); if (!folder?.[0]) return;
        const name = await vscode.window.showInputBox({ prompt: 'Nombre del proyecto TITAN', validateInput: value => /^[A-Za-z_][A-Za-z0-9_-]*$/.test(value) ? undefined : 'Usa letras, números, _ o -, comenzando por una letra o _.' }); if (!name) return;
        const task = new vscode.Task({ type: 'titan', task: 'new' }, vscode.TaskScope.Workspace, 'Create TITAN project', 'TITAN', new vscode.ProcessExecution(String(setting('compiler.path')), ['new', vscode.Uri.joinPath(folder[0], name).fsPath])); await vscode.tasks.executeTask(task);
    }));
}
export async function deactivate(): Promise<void> { await client?.stop(); }
