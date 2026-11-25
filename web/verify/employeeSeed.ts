import type { Employee, EmployeeRead, Face } from '../src/core/generated/types.generated';
import { employeeRead } from './records';

// tagOf words a made-up employee's tag as the server does (model.Employee.Tag), for seeds that hold no server: its code and
// what it was hired from.
export const tagOf = (e: Employee) => `E-${e.number} (${e.template})`;

// shownOf words a made-up employee as the server does (model.Employee.Shown): its name when it has one, then its tag.
export const shownOf = (e: Employee) => `${e.label ? `${e.label} · ` : ''}${tagOf(e)}`;

// wordedRead is a made-up employee as a channel reads it: worded as the server words it, with the face the seed gives it.
export const wordedRead = (e: Employee, face: Face): EmployeeRead => employeeRead(e, { shown: shownOf(e), tag: tagOf(e), face });
