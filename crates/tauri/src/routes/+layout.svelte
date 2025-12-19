<script lang="ts">
    import "../app.css";
    import { onMount } from "svelte";
    import { listen } from "@tauri-apps/api/event";
    import { open, save } from "@tauri-apps/plugin-dialog";
    import { sessionStore } from "$lib/stores/session.svelte";
    import { dialogPathStore } from "$lib/stores/dialog-paths.svelte";

    onMount(() => {
        // Listen for menu events from the native menu
        const unlisten = listen<string>("menu-event", async (event) => {
            const menuId = event.payload;

            switch (menuId) {
                case "open_project":
                    await handleOpenProject();
                    break;
                case "save":
                    await handleSave();
                    break;
                case "save_as":
                    await handleSaveAs();
                    break;
                case "render":
                    await handleRender();
                    break;
            }
        });

        return () => {
            unlisten.then((fn) => fn());
        };
    });

    async function handleOpenProject() {
        try {
            const selected = await open({
                multiple: false,
                defaultPath: dialogPathStore.getPath("projectOpen"),
                filters: [
                    {
                        name: "DAW Project",
                        extensions: ["dawproj"],
                    },
                ],
            });

            if (selected && typeof selected === "string") {
                await sessionStore.loadProject(selected);
                dialogPathStore.setPath("projectOpen", selected);
            }
        } catch (err) {
            const errorMsg = err instanceof Error ? err.message : String(err);
            console.error("Failed to open project:", err);
            alert(`Failed to open project: ${errorMsg}`);
        }
    }

    async function handleSave() {
        try {
            await sessionStore.save();
        } catch (err) {
            const errorMsg = err instanceof Error ? err.message : String(err);
            console.error("Failed to save project:", err);
            alert(`Failed to save project: ${errorMsg}`);
        }
    }

    async function handleSaveAs() {
        try {
            const session = sessionStore.session;
            const defaultFileName = session
                ? `${session.name}.dawproj`
                : undefined;

            const selected = await save({
                defaultPath: dialogPathStore.buildPath(
                    "saveAs",
                    defaultFileName,
                ),
                filters: [
                    {
                        name: "DAW Project",
                        extensions: ["dawproj"],
                    },
                ],
            });

            if (selected) {
                await sessionStore.saveAs(selected);
                dialogPathStore.setPath("saveAs", selected);
            }
        } catch (err) {
            const errorMsg = err instanceof Error ? err.message : String(err);
            console.error("Failed to save project:", err);
            alert(`Failed to save project: ${errorMsg}`);
        }
    }

    async function handleRender() {
        try {
            const session = sessionStore.session;
            const defaultFileName = session
                ? `${session.name}.wav`
                : "output.wav";

            const selected = await save({
                defaultPath: dialogPathStore.buildPath(
                    "render",
                    defaultFileName,
                ),
                filters: [
                    {
                        name: "WAV Audio",
                        extensions: ["wav"],
                    },
                ],
            });

            if (selected) {
                await sessionStore.render(selected);
                dialogPathStore.setPath("render", selected);
                alert("Render complete!");
            }
        } catch (err) {
            const errorMsg = err instanceof Error ? err.message : String(err);
            console.error("Failed to render project:", err);
            alert(`Failed to render project: ${errorMsg}`);
        }
    }
</script>

<slot />
