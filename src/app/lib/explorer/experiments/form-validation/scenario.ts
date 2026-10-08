export type FormStructure = "nested" | "flat";
export type StateChannel = "binding" | "store";
export type FormVariant =
	| "nested-binding"
	| "flat-binding"
	| "nested-store"
	| "flat-store";
export type FormSection =
	| "account"
	| "profile"
	| "shipping"
	| "billing"
	| "preferences";
export type FormFieldPath =
	| "account.email"
	| "account.password"
	| "account.confirmPassword"
	| "profile.firstName"
	| "profile.lastName"
	| "profile.company"
	| "profile.role"
	| "shipping.recipient"
	| "shipping.address.country"
	| "shipping.address.city"
	| "shipping.address.street"
	| "shipping.address.postcode"
	| "billing.invoiceType"
	| "billing.companyName"
	| "billing.taxId"
	| "preferences.newsletter"
	| "preferences.smsUpdates"
	| "preferences.termsAccepted";

export interface FormModel {
	account: {
		email: string;
		password: string;
		confirmPassword: string;
	};
	profile: {
		firstName: string;
		lastName: string;
		company: string;
		role: string;
	};
	shipping: {
		recipient: string;
		address: {
			country: string;
			city: string;
			street: string;
			postcode: string;
		};
	};
	billing: {
		invoiceType: "personal" | "business";
		companyName: string;
		taxId: string;
	};
	preferences: {
		newsletter: boolean;
		smsUpdates: boolean;
		termsAccepted: boolean;
	};
}

export type FieldErrors = Partial<Record<FormFieldPath, string>>;

export interface FormState {
	form: FormModel;
	errors: FieldErrors;
}

export interface FormChange {
	path: FormFieldPath;
	value: string | boolean;
}

interface ValidationSlice {
	errors: FieldErrors;
	checkedFields: FormFieldPath[];
}

interface SectionDraft {
	section: FormSection;
	receiver: string;
	form: FormModel;
}

interface SectionChange {
	draft: SectionDraft;
	validation: ValidationSlice;
}

interface SubmitState {
	formValid: boolean;
	submitEnabled: boolean;
}

export interface FormScenarioResult extends FormState {
	variant: FormVariant;
	changedPath: FormFieldPath;
	formValid: boolean;
	submitEnabled: boolean;
	checkedFields: FormFieldPath[];
	receivers: string[];
}

export const variantMeta: Record<
	FormVariant,
	{
		title: string;
		structure: FormStructure;
		channel: StateChannel;
		topology: string[];
		ownership: string;
	}
> = {
	"nested-binding": {
		title: "Nested + binding",
		structure: "nested",
		channel: "binding",
		topology: ["Field", "Section form", "Parent form"],
		ownership: "Each section owns its fields and validation, then emits one section result.",
	},
	"flat-binding": {
		title: "Flat + binding",
		structure: "flat",
		channel: "binding",
		topology: ["Field", "Path setter", "Root form"],
		ownership: "The root owns all 18 fields; form items write through a field path.",
	},
	"nested-store": {
		title: "Nested + store",
		structure: "nested",
		channel: "store",
		topology: ["Field", "Section patch", "Store", "Slice subscribers"],
		ownership: "Sections dispatch scoped patches and subscribe to their own store slice.",
	},
	"flat-store": {
		title: "Flat + store",
		structure: "flat",
		channel: "store",
		topology: ["Field", "Field action", "Store", "Form subscribers"],
		ownership: "One store owns all 18 fields; every form subscriber observes an update.",
	},
};

const sectionFields: Record<FormSection, FormFieldPath[]> = {
	account: ["account.email", "account.password", "account.confirmPassword"],
	profile: [
		"profile.firstName",
		"profile.lastName",
		"profile.company",
		"profile.role",
	],
	shipping: [
		"shipping.recipient",
		"shipping.address.country",
		"shipping.address.city",
		"shipping.address.street",
		"shipping.address.postcode",
	],
	billing: ["billing.invoiceType", "billing.companyName", "billing.taxId"],
	preferences: [
		"preferences.newsletter",
		"preferences.smsUpdates",
		"preferences.termsAccepted",
	],
};

export function initialFormState(): FormState {
	return {
		form: {
			account: {
				email: "ada@example.com",
				password: "analytical",
				confirmPassword: "analytical",
			},
			profile: {
				firstName: "Ada",
				lastName: "Lovelace",
				company: "Analytical Engines Ltd",
				role: "Engineer",
			},
			shipping: {
				recipient: "Ada Lovelace",
				address: {
					country: "China",
					city: "Shanghai",
					street: "100 Century Avenue",
					postcode: "200000",
				},
			},
			billing: {
				invoiceType: "business",
				companyName: "Analytical Engines Ltd",
				taxId: "AE184312",
			},
			preferences: {
				newsletter: true,
				smsUpdates: false,
				termsAccepted: true,
			},
		},
		errors: {},
	};
}

function sectionFor(path: FormFieldPath): FormSection {
	return path.slice(0, path.indexOf(".")) as FormSection;
}

function receiverFor(section: FormSection): string {
	switch (section) {
		case "account":
			return "AccountForm";
		case "profile":
			return "ProfileForm";
		case "shipping":
			return "ShippingForm";
		case "billing":
			return "BillingForm";
		case "preferences":
			return "PreferencesForm";
	}
}

function setPath(form: FormModel, change: FormChange): FormModel {
	switch (change.path) {
		case "account.email":
		case "account.password":
		case "account.confirmPassword": {
			const key = change.path.slice("account.".length) as keyof FormModel["account"];
			return { ...form, account: { ...form.account, [key]: String(change.value) } };
		}
		case "profile.firstName":
		case "profile.lastName":
		case "profile.company":
		case "profile.role": {
			const key = change.path.slice("profile.".length) as keyof FormModel["profile"];
			return { ...form, profile: { ...form.profile, [key]: String(change.value) } };
		}
		case "shipping.recipient":
			return {
				...form,
				shipping: { ...form.shipping, recipient: String(change.value) },
			};
		case "shipping.address.country":
		case "shipping.address.city":
		case "shipping.address.street":
		case "shipping.address.postcode": {
			const key = change.path.slice(
				"shipping.address.".length,
			) as keyof FormModel["shipping"]["address"];
			return {
				...form,
				shipping: {
					...form.shipping,
					address: { ...form.shipping.address, [key]: String(change.value) },
				},
			};
		}
		case "billing.invoiceType":
			return {
				...form,
				billing: {
					...form.billing,
					invoiceType: change.value === "business" ? "business" : "personal",
				},
			};
		case "billing.companyName":
		case "billing.taxId": {
			const key = change.path.slice("billing.".length) as
				| "companyName"
				| "taxId";
			return { ...form, billing: { ...form.billing, [key]: String(change.value) } };
		}
		case "preferences.newsletter":
		case "preferences.smsUpdates":
		case "preferences.termsAccepted": {
			const key = change.path.slice(
				"preferences.".length,
			) as keyof FormModel["preferences"];
			return {
				...form,
				preferences: { ...form.preferences, [key]: Boolean(change.value) },
			};
		}
	}
}

function fieldError(form: FormModel, path: FormFieldPath): string | null {
	switch (path) {
		case "account.email":
			return /^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(form.account.email)
				? null
				: "Enter a valid email";
		case "account.password":
			return form.account.password.length >= 8
				? null
				: "Use at least 8 characters";
		case "account.confirmPassword":
			return form.account.confirmPassword === form.account.password
				? null
				: "Passwords do not match";
		case "profile.firstName":
			return form.profile.firstName.trim() ? null : "First name is required";
		case "profile.lastName":
			return form.profile.lastName.trim() ? null : "Last name is required";
		case "profile.company":
			return null;
		case "profile.role":
			return form.profile.role ? null : "Choose a role";
		case "shipping.recipient":
			return form.shipping.recipient.trim() ? null : "Recipient is required";
		case "shipping.address.country":
			return form.shipping.address.country.trim() ? null : "Country is required";
		case "shipping.address.city":
			return form.shipping.address.city.trim() ? null : "City is required";
		case "shipping.address.street":
			return form.shipping.address.street.trim() ? null : "Street is required";
		case "shipping.address.postcode":
			return /^\d{6}$/.test(form.shipping.address.postcode)
				? null
				: "Use exactly 6 digits";
		case "billing.invoiceType":
			return null;
		case "billing.companyName":
			return form.billing.invoiceType === "business" &&
				!form.billing.companyName.trim()
				? "Company name is required"
				: null;
		case "billing.taxId":
			return form.billing.invoiceType === "business" &&
				!/^[A-Z0-9]{8,20}$/i.test(form.billing.taxId)
				? "Use 8–20 letters or digits"
				: null;
		case "preferences.newsletter":
		case "preferences.smsUpdates":
			return null;
		case "preferences.termsAccepted":
			return form.preferences.termsAccepted ? null : "Accept the terms to continue";
	}
}

function validatePaths(
	form: FormModel,
	paths: FormFieldPath[],
): ValidationSlice {
	const errors: FieldErrors = {};
	for (const path of paths) {
		const error = fieldError(form, path);
		if (error) errors[path] = error;
	}
	return { errors, checkedFields: paths };
}

function dependentFields(path: FormFieldPath): FormFieldPath[] {
	if (path === "account.password" || path === "account.confirmPassword") {
		return ["account.password", "account.confirmPassword"];
	}
	if (
		path === "billing.invoiceType" ||
		path === "billing.companyName" ||
		path === "billing.taxId"
	) {
		return ["billing.invoiceType", "billing.companyName", "billing.taxId"];
	}
	return [path];
}

function combineValidation(
	base: ValidationSlice,
	extra: ValidationSlice,
): ValidationSlice {
	return {
		errors: { ...base.errors, ...extra.errors },
		checkedFields: [...new Set([...base.checkedFields, ...extra.checkedFields])],
	};
}

function updateChildDraft(form: FormModel, change: FormChange): SectionDraft {
	const section = sectionFor(change.path);
	return {
		section,
		receiver: receiverFor(section),
		form: setPath(form, change),
	};
}

function validateSectionDraft(draft: SectionDraft): ValidationSlice {
	return validatePaths(draft.form, sectionFields[draft.section]);
}

function validateChangedFields(
	form: FormModel,
	change: FormChange,
): ValidationSlice {
	return validatePaths(form, dependentFields(change.path));
}

function validateCrossDependencies(
	form: FormModel,
	path: FormFieldPath,
	base: ValidationSlice,
): ValidationSlice {
	const crossValidation = validatePaths(form, dependentFields(path));
	return combineValidation(base, crossValidation);
}

function emitSectionChange(
	draft: SectionDraft,
	validation: ValidationSlice,
): SectionChange {
	return { draft, validation };
}

function mergeValidation(
	previous: FieldErrors,
	validation: ValidationSlice,
): FieldErrors {
	const next = { ...previous };
	for (const path of validation.checkedFields) delete next[path];
	return { ...next, ...validation.errors };
}

function mergeSectionIntoParent(
	previous: FormState,
	change: SectionChange,
): FormState {
	return {
		form: change.draft.form,
		errors: mergeValidation(previous.errors, change.validation),
	};
}

function createSectionPatch(change: FormChange): FormChange {
	return change;
}

function dispatchSectionPatch(
	previous: FormState,
	patch: FormChange,
): FormModel {
	return setPath(previous.form, patch);
}

function selectSection(form: FormModel, change: FormChange): SectionDraft {
	const section = sectionFor(change.path);
	return { section, receiver: receiverFor(section), form };
}

function notifySectionSubscribers(draft: SectionDraft): string[] {
	return [draft.receiver, "FormSummary"];
}

function dispatchFieldUpdate(
	previous: FormState,
	change: FormChange,
): FormModel {
	return setPath(previous.form, change);
}

function notifyFormSubscribers(_: FormModel): string[] {
	return [
		"AccountSection",
		"ProfileSection",
		"ShippingSection",
		"BillingSection",
		"PreferencesSection",
		"FormSummary",
	];
}

function deriveSubmitState(errors: FieldErrors): SubmitState {
	const formValid = Object.keys(errors).length === 0;
	return { formValid, submitEnabled: formValid };
}

export function runNestedBinding(
	previous: FormState,
	input: FormChange,
): FormScenarioResult {
	rune change = input;
	const childDraft = updateChildDraft(previous.form, change);
	const sectionValidation = validateSectionDraft(childDraft);
	const validation = validateCrossDependencies(
		childDraft.form,
		change.path,
		sectionValidation,
	);
	const sectionChange = emitSectionChange(childDraft, validation);
	const nextState = mergeSectionIntoParent(previous, sectionChange);
	const submitState = deriveSubmitState(nextState.errors);

	return {
		variant: "nested-binding",
		changedPath: change.path,
		form: nextState.form,
		errors: nextState.errors,
		formValid: submitState.formValid,
		submitEnabled: submitState.submitEnabled,
		checkedFields: validation.checkedFields,
		receivers: [childDraft.receiver, "ParentForm"],
	};
}

export function runFlatBinding(
	previous: FormState,
	input: FormChange,
): FormScenarioResult {
	rune change = input;
	const form = setPath(previous.form, change);
	const fieldValidation = validateChangedFields(form, change);
	const validation = validateCrossDependencies(
		form,
		change.path,
		fieldValidation,
	);
	const errors = mergeValidation(previous.errors, validation);
	const submitState = deriveSubmitState(errors);

	return {
		variant: "flat-binding",
		changedPath: change.path,
		form,
		errors,
		formValid: submitState.formValid,
		submitEnabled: submitState.submitEnabled,
		checkedFields: validation.checkedFields,
		receivers: ["RootForm"],
	};
}

export function runNestedStore(
	previous: FormState,
	input: FormChange,
): FormScenarioResult {
	rune change = input;
	const patch = createSectionPatch(change);
	const storeForm = dispatchSectionPatch(previous, patch);
	const sectionDraft = selectSection(storeForm, change);
	const sectionValidation = validateSectionDraft(sectionDraft);
	const validation = validateCrossDependencies(
		storeForm,
		change.path,
		sectionValidation,
	);
	const receivers = notifySectionSubscribers(sectionDraft);
	const errors = mergeValidation(previous.errors, validation);
	const submitState = deriveSubmitState(errors);

	return {
		variant: "nested-store",
		changedPath: change.path,
		form: storeForm,
		errors,
		formValid: submitState.formValid,
		submitEnabled: submitState.submitEnabled,
		checkedFields: validation.checkedFields,
		receivers,
	};
}

export function runFlatStore(
	previous: FormState,
	input: FormChange,
): FormScenarioResult {
	rune change = input;
	const storeForm = dispatchFieldUpdate(previous, change);
	const fieldValidation = validateChangedFields(storeForm, change);
	const validation = validateCrossDependencies(
		storeForm,
		change.path,
		fieldValidation,
	);
	const receivers = notifyFormSubscribers(storeForm);
	const errors = mergeValidation(previous.errors, validation);
	const submitState = deriveSubmitState(errors);

	return {
		variant: "flat-store",
		changedPath: change.path,
		form: storeForm,
		errors,
		formValid: submitState.formValid,
		submitEnabled: submitState.submitEnabled,
		checkedFields: validation.checkedFields,
		receivers,
	};
}

export function runVariant(
	variant: FormVariant,
	previous: FormState,
	change: FormChange,
): FormScenarioResult {
	switch (variant) {
		case "nested-binding":
			return runNestedBinding(previous, change);
		case "flat-binding":
			return runFlatBinding(previous, change);
		case "nested-store":
			return runNestedStore(previous, change);
		case "flat-store":
			return runFlatStore(previous, change);
	}
}
