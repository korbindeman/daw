<script lang="ts">
    import { sessionStore } from "$lib/stores/session.svelte";

    // Reactive values
    const session = $derived(sessionStore.session);
    const musicalTime = $derived(
        session
            ? sessionStore.ticksToMusicalTime(session.currentTick)
            : "0:0:0",
    );
</script>

<header class="bg-gray-900 text-white border-b border-gray-700">
    <div class="flex items-center justify-between px-4 py-3">
        <!-- Left: Project name -->
        <div class="flex items-center gap-4">
            {#if session}
                <div class="text-sm">
                    <span class="text-gray-400">Project:</span>
                    <span class="ml-2 font-medium">{session.name}</span>
                </div>
            {/if}
        </div>

        <!-- Center: Musical time indicator -->
        {#if session}
            <div class="flex items-center gap-6">
                <div class="text-center">
                    <div class="text-xs text-gray-400 mb-1">Position</div>
                    <div class="font-mono text-lg font-semibold tabular-nums">
                        {musicalTime}
                    </div>
                </div>
            </div>
        {/if}

        <!-- Right: Tempo and time signature -->
        {#if session}
            <div class="flex items-center gap-6">
                <div class="text-center">
                    <div class="text-xs text-gray-400 mb-1">Tempo</div>
                    <div class="font-mono text-lg font-semibold tabular-nums">
                        {session.tempo.toFixed(1)} BPM
                    </div>
                </div>

                <div class="text-center">
                    <div class="text-xs text-gray-400 mb-1">Time Signature</div>
                    <div class="font-mono text-lg font-semibold tabular-nums">
                        {session.timeSignature.numerator}/{session.timeSignature
                            .denominator}
                    </div>
                </div>
            </div>
        {/if}
    </div>
</header>
