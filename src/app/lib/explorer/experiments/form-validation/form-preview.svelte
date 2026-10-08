<script lang="ts">
	import CheckIcon from "@lucide/svelte/icons/check";
	import LockIcon from "@lucide/svelte/icons/lock";
	import { Button } from "$lib/components/ui/button/index.js";
	import { Input } from "$lib/components/ui/input/index.js";
	import type {
		FieldErrors,
		FormFieldPath,
		FormModel,
	} from "./scenario.js";

	interface Props {
		form: FormModel;
		errors: FieldErrors;
		activePath: FormFieldPath;
		submitEnabled: boolean;
		onfieldchange: (path: FormFieldPath, value: string | boolean) => void;
	}

	let { form, errors, activePath, submitEnabled, onfieldchange }: Props = $props();
	const errorCount = $derived(Object.keys(errors).length);

	function fieldClass(path: FormFieldPath): string {
		return activePath === path ? "border-chart-2 ring-2 ring-chart-2/15" : "";
	}
</script>

<form class="space-y-4 p-4" onsubmit={(event) => event.preventDefault()}>
	<div class="flex items-center justify-between gap-3 rounded-lg border bg-background px-3 py-2.5">
		<div>
			<div class="text-xs font-semibold">Workspace access request</div>
			<div class="mt-0.5 text-[10px] text-muted-foreground">18 fields · 5 sections · conditional billing</div>
		</div>
		<span
			class={{
				"rounded-full px-2 py-1 text-[10px] font-medium": true,
				"bg-emerald-500/10 text-emerald-700 dark:text-emerald-300": errorCount === 0,
				"bg-destructive/10 text-destructive": errorCount > 0,
			}}
		>
			{errorCount === 0 ? "Ready to submit" : `${errorCount} ${errorCount === 1 ? "error" : "errors"}`}
		</span>
	</div>

	<fieldset class="rounded-lg border bg-background p-3">
		<legend class="px-1 text-xs font-semibold">1. Account security</legend>
		<p class="mb-3 text-[10px] text-muted-foreground">Password and confirmation validate as a dependency pair.</p>
		<div class="grid gap-3 sm:grid-cols-2">
			<label class="sm:col-span-2">
				<span class="mb-1 block text-[10px] font-medium">Email address</span>
				<Input
					type="email"
					value={form.account.email}
					class={fieldClass("account.email")}
					aria-invalid={Boolean(errors["account.email"])}
					oninput={(event) => onfieldchange("account.email", event.currentTarget.value)}
				/>
				{#if errors["account.email"]}<span class="mt-1 block text-[10px] text-destructive">{errors["account.email"]}</span>{/if}
			</label>
			<label>
				<span class="mb-1 block text-[10px] font-medium">Password</span>
				<Input
					type="password"
					value={form.account.password}
					class={fieldClass("account.password")}
					aria-invalid={Boolean(errors["account.password"])}
					oninput={(event) => onfieldchange("account.password", event.currentTarget.value)}
				/>
				{#if errors["account.password"]}<span class="mt-1 block text-[10px] text-destructive">{errors["account.password"]}</span>{/if}
			</label>
			<label>
				<span class="mb-1 block text-[10px] font-medium">Confirm password</span>
				<Input
					type="password"
					value={form.account.confirmPassword}
					class={fieldClass("account.confirmPassword")}
					aria-invalid={Boolean(errors["account.confirmPassword"])}
					oninput={(event) => onfieldchange("account.confirmPassword", event.currentTarget.value)}
				/>
				{#if errors["account.confirmPassword"]}<span class="mt-1 block text-[10px] text-destructive">{errors["account.confirmPassword"]}</span>{/if}
			</label>
		</div>
	</fieldset>

	<fieldset class="rounded-lg border bg-background p-3">
		<legend class="px-1 text-xs font-semibold">2. Profile</legend>
		<p class="mb-3 text-[10px] text-muted-foreground">Identity fields form one logical profile section.</p>
		<div class="grid gap-3 sm:grid-cols-2">
			<label>
				<span class="mb-1 block text-[10px] font-medium">First name</span>
				<Input
					value={form.profile.firstName}
					class={fieldClass("profile.firstName")}
					aria-invalid={Boolean(errors["profile.firstName"])}
					oninput={(event) => onfieldchange("profile.firstName", event.currentTarget.value)}
				/>
				{#if errors["profile.firstName"]}<span class="mt-1 block text-[10px] text-destructive">{errors["profile.firstName"]}</span>{/if}
			</label>
			<label>
				<span class="mb-1 block text-[10px] font-medium">Last name</span>
				<Input
					value={form.profile.lastName}
					class={fieldClass("profile.lastName")}
					aria-invalid={Boolean(errors["profile.lastName"])}
					oninput={(event) => onfieldchange("profile.lastName", event.currentTarget.value)}
				/>
				{#if errors["profile.lastName"]}<span class="mt-1 block text-[10px] text-destructive">{errors["profile.lastName"]}</span>{/if}
			</label>
			<label>
				<span class="mb-1 block text-[10px] font-medium">Company</span>
				<Input
					value={form.profile.company}
					class={fieldClass("profile.company")}
					oninput={(event) => onfieldchange("profile.company", event.currentTarget.value)}
				/>
			</label>
			<label>
				<span class="mb-1 block text-[10px] font-medium">Role</span>
				<select
					class="h-9 w-full rounded-md border border-input bg-transparent px-2.5 text-sm outline-none focus-visible:ring-3 focus-visible:ring-ring/50 {fieldClass("profile.role")}"
					value={form.profile.role}
					aria-invalid={Boolean(errors["profile.role"])}
					onchange={(event) => onfieldchange("profile.role", event.currentTarget.value)}
				>
					<option value="">Select a role</option>
					<option>Engineer</option>
					<option>Designer</option>
					<option>Product manager</option>
					<option>Operations</option>
				</select>
				{#if errors["profile.role"]}<span class="mt-1 block text-[10px] text-destructive">{errors["profile.role"]}</span>{/if}
			</label>
		</div>
	</fieldset>

	<fieldset class="rounded-lg border bg-background p-3">
		<legend class="px-1 text-xs font-semibold">3. Shipping address</legend>
		<p class="mb-3 text-[10px] text-muted-foreground">A nested address object gives path propagation more than one level.</p>
		<div class="grid gap-3 sm:grid-cols-2">
			<label class="sm:col-span-2">
				<span class="mb-1 block text-[10px] font-medium">Recipient</span>
				<Input
					value={form.shipping.recipient}
					class={fieldClass("shipping.recipient")}
					aria-invalid={Boolean(errors["shipping.recipient"])}
					oninput={(event) => onfieldchange("shipping.recipient", event.currentTarget.value)}
				/>
				{#if errors["shipping.recipient"]}<span class="mt-1 block text-[10px] text-destructive">{errors["shipping.recipient"]}</span>{/if}
			</label>
			<label>
				<span class="mb-1 block text-[10px] font-medium">Country</span>
				<Input
					value={form.shipping.address.country}
					class={fieldClass("shipping.address.country")}
					aria-invalid={Boolean(errors["shipping.address.country"])}
					oninput={(event) => onfieldchange("shipping.address.country", event.currentTarget.value)}
				/>
				{#if errors["shipping.address.country"]}<span class="mt-1 block text-[10px] text-destructive">{errors["shipping.address.country"]}</span>{/if}
			</label>
			<label>
				<span class="mb-1 block text-[10px] font-medium">City</span>
				<Input
					value={form.shipping.address.city}
					class={fieldClass("shipping.address.city")}
					aria-invalid={Boolean(errors["shipping.address.city"])}
					oninput={(event) => onfieldchange("shipping.address.city", event.currentTarget.value)}
				/>
				{#if errors["shipping.address.city"]}<span class="mt-1 block text-[10px] text-destructive">{errors["shipping.address.city"]}</span>{/if}
			</label>
			<label class="sm:col-span-2">
				<span class="mb-1 block text-[10px] font-medium">Street address</span>
				<Input
					value={form.shipping.address.street}
					class={fieldClass("shipping.address.street")}
					aria-invalid={Boolean(errors["shipping.address.street"])}
					oninput={(event) => onfieldchange("shipping.address.street", event.currentTarget.value)}
				/>
				{#if errors["shipping.address.street"]}<span class="mt-1 block text-[10px] text-destructive">{errors["shipping.address.street"]}</span>{/if}
			</label>
			<label>
				<span class="mb-1 block text-[10px] font-medium">Postcode</span>
				<Input
					value={form.shipping.address.postcode}
					class={fieldClass("shipping.address.postcode")}
					inputmode="numeric"
					aria-invalid={Boolean(errors["shipping.address.postcode"])}
					oninput={(event) => onfieldchange("shipping.address.postcode", event.currentTarget.value)}
				/>
				{#if errors["shipping.address.postcode"]}<span class="mt-1 block text-[10px] text-destructive">{errors["shipping.address.postcode"]}</span>{/if}
			</label>
		</div>
	</fieldset>

	<fieldset class="rounded-lg border bg-background p-3">
		<legend class="px-1 text-xs font-semibold">4. Billing</legend>
		<p class="mb-3 text-[10px] text-muted-foreground">Business invoice fields become required together.</p>
		<div class="mb-3 grid grid-cols-2 gap-1 rounded-lg bg-muted/60 p-1">
			{#each ["personal", "business"] as invoiceType (invoiceType)}
				<button
					type="button"
					class={{
						"rounded-md px-2.5 py-1.5 text-xs capitalize transition-colors": true,
						"bg-background font-medium shadow-sm": form.billing.invoiceType === invoiceType,
						"text-muted-foreground": form.billing.invoiceType !== invoiceType,
					}}
					onclick={() => onfieldchange("billing.invoiceType", invoiceType)}
				>
					{invoiceType}
				</button>
			{/each}
		</div>
		{#if form.billing.invoiceType === "business"}
			<div class="grid gap-3 sm:grid-cols-2">
				<label>
					<span class="mb-1 block text-[10px] font-medium">Legal company name</span>
					<Input
						value={form.billing.companyName}
						class={fieldClass("billing.companyName")}
						aria-invalid={Boolean(errors["billing.companyName"])}
						oninput={(event) => onfieldchange("billing.companyName", event.currentTarget.value)}
					/>
					{#if errors["billing.companyName"]}<span class="mt-1 block text-[10px] text-destructive">{errors["billing.companyName"]}</span>{/if}
				</label>
				<label>
					<span class="mb-1 block text-[10px] font-medium">Tax ID</span>
					<Input
						value={form.billing.taxId}
						class={fieldClass("billing.taxId")}
						aria-invalid={Boolean(errors["billing.taxId"])}
						oninput={(event) => onfieldchange("billing.taxId", event.currentTarget.value)}
					/>
					{#if errors["billing.taxId"]}<span class="mt-1 block text-[10px] text-destructive">{errors["billing.taxId"]}</span>{/if}
				</label>
			</div>
		{:else}
			<div class="rounded-md border border-dashed px-3 py-4 text-center text-[10px] text-muted-foreground">
				Company name and tax ID are excluded from validation.
			</div>
		{/if}
	</fieldset>

	<fieldset class="rounded-lg border bg-background p-3">
		<legend class="px-1 text-xs font-semibold">5. Preferences</legend>
		<div class="space-y-2">
			<label class="flex cursor-pointer items-start gap-2 rounded-md border px-3 py-2">
				<input
					type="checkbox"
					class="mt-0.5 size-3.5 accent-foreground"
					checked={form.preferences.newsletter}
					onchange={(event) => onfieldchange("preferences.newsletter", event.currentTarget.checked)}
				/>
				<span><span class="block text-xs font-medium">Product newsletter</span><span class="block text-[10px] text-muted-foreground">Monthly product and release notes.</span></span>
			</label>
			<label class="flex cursor-pointer items-start gap-2 rounded-md border px-3 py-2">
				<input
					type="checkbox"
					class="mt-0.5 size-3.5 accent-foreground"
					checked={form.preferences.smsUpdates}
					onchange={(event) => onfieldchange("preferences.smsUpdates", event.currentTarget.checked)}
				/>
				<span><span class="block text-xs font-medium">SMS delivery updates</span><span class="block text-[10px] text-muted-foreground">Receive shipment status on mobile.</span></span>
			</label>
			<label class="flex cursor-pointer items-start gap-2 rounded-md border px-3 py-2 {activePath === "preferences.termsAccepted" ? "border-chart-2 ring-2 ring-chart-2/15" : ""}">
				<input
					type="checkbox"
					class="mt-0.5 size-3.5 accent-foreground"
					checked={form.preferences.termsAccepted}
					onchange={(event) => onfieldchange("preferences.termsAccepted", event.currentTarget.checked)}
				/>
				<span><span class="block text-xs font-medium">Accept service terms</span><span class="block text-[10px] text-muted-foreground">Required before the request can be submitted.</span>{#if errors["preferences.termsAccepted"]}<span class="mt-1 block text-[10px] text-destructive">{errors["preferences.termsAccepted"]}</span>{/if}</span>
			</label>
		</div>
	</fieldset>

	<div class="sticky bottom-0 flex items-center justify-between gap-3 rounded-lg border bg-background/95 p-3 shadow-lg backdrop-blur">
		<div class="flex items-center gap-2 text-[10px] text-muted-foreground">
			{#if submitEnabled}<CheckIcon class="size-3.5 text-emerald-600" />All validation passed{:else}<LockIcon class="size-3.5" />Resolve validation errors{/if}
		</div>
		<Button type="submit" size="sm" disabled={!submitEnabled}>Request access</Button>
	</div>
</form>
