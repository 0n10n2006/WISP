import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "../../src/App";
import { useSettings } from "../../src/state/settings";
import { setupMockHost } from "../helpers";

describe("settings window", () => {
  it("walks through first-run setup", async () => {
    await setupMockHost({ windowKind: "settings" });
    const user = userEvent.setup();
    render(<App />);

    expect(await screen.findByRole("heading", { name: /meet wisp/i })).toBeInTheDocument();
    expect(screen.getByText(/free tier, google may use prompts/i)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /continue/i }));

    const input = screen.getByLabelText(/api key/i);
    await user.type(input, "bad-key-000000000000000000");
    await user.click(screen.getByRole("button", { name: /test & save/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/rejected/i);

    await user.clear(input);
    await user.type(input, "AIzaGOOD-key-0000000000000000000");
    await user.click(screen.getByRole("button", { name: /test & save/i }));
    expect(await screen.findByRole("heading", { name: /choose a model/i })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: /finish setup/i }));
    expect(await screen.findByRole("heading", { name: /you're set/i })).toBeInTheDocument();
    expect(useSettings.getState().snapshot?.settings.onboardingComplete).toBe(true);
    expect(useSettings.getState().snapshot?.hasApiKey).toBe(true);
    await user.click(screen.getByRole("button", { name: /done/i }));
    expect(await screen.findByRole("heading", { name: "General" })).toBeInTheDocument();
  });

  it("switches theme immediately", async () => {
    await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("radio", { name: "Light" }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("light"));
    await user.click(screen.getByRole("radio", { name: "Dark" }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
  });

  it("rejects conflicting shortcuts and can reset them", async () => {
    await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Shortcuts" }));
    const dictation = screen.getByRole("button", { name: /^dictation:/i });
    await user.click(dictation);
    await user.keyboard("{Control>}{Alt>}[Space]{/Alt}{/Control}");
    await user.click(screen.getByRole("button", { name: /save shortcuts/i }));
    expect(await screen.findByRole("alert")).toHaveTextContent(/assigned to both/i);
    await user.click(screen.getByRole("button", { name: /reset to defaults/i }));
    expect(await screen.findByText(/reset to defaults\./i)).toBeInTheDocument();
    expect(
      within(screen.getByRole("button", { name: /^dictation:/i })).getByText("Ctrl+Shift+D"),
    ).toBeInTheDocument();
  });

  it("asks before removing the API key", async () => {
    await setupMockHost({ windowKind: "settings", withKey: true, onboarded: true });
    const user = userEvent.setup();
    render(<App />);
    await user.click(await screen.findByRole("tab", { name: "Model" }));
    await user.click(screen.getByRole("button", { name: /remove key/i }));
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Remove key" }));
    await waitFor(() => expect(useSettings.getState().snapshot?.hasApiKey).toBe(false));
  });
});
