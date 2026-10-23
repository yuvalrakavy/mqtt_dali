// node_modules/preact/dist/preact.mjs
var e, t, n, r, i, a, o, s, c, l, u = {}, d = [], f = /^m(i|n|o|s|text|space)$/, p = Array.isArray, m = d.slice, h = Object.assign;
function g(e) {
	e && e.parentNode && e.remove();
}
function _(n, r, i, a, o) {
	var s = {
		type: n,
		props: r,
		key: i,
		ref: a,
		__k: null,
		__: null,
		__b: 0,
		__e: null,
		__c: null,
		constructor: void 0,
		__v: o || ++t,
		__i: -1,
		__u: 0
	};
	return !o && e.vnode && e.vnode(s), s;
}
function v(e) {
	return e.children;
}
function y(e, t) {
	this.props = e, this.context = t, this.__g = 0;
}
function b(e, t) {
	if (t == null) return e.__ ? b(e.__, e.__i + 1) : null;
	for (var n; t < e.__k.length; t++) if ((n = e.__k[t]) && n.__e) return n.__e;
	return typeof e.type != "function" || e.props.__P ? null : b(e);
}
function x(e) {
	if ((e = e.__) && e.__c && !e.props.__P) return e.__e = null, e.__k.some(function(t) {
		return t && (e.__e = t.__e);
	}), x(e);
}
function S(t) {
	(8 & t.__g || !(t.__g |= 8) || !n.push(t) || i++) && r == e.debounceRendering || ((r = e.debounceRendering) || queueMicrotask)(C);
}
function C() {
	var t, r, o, s, c, l, u, d, f;
	try {
		for (r = 1; n.length;) n.length > r && n.sort(a), t = n.shift(), r = n.length, 8 & t.__g && (s = void 0, c = void 0, l = (c = (o = t).__v).__e, u = [], d = [], (f = o.__P) && ((s = h({ constructor: void 0 }, c)).__v = c.__v + 1, e.vnode && e.vnode(s), j(f, s, c, o.__n, f.namespaceURI, 32 & c.__u ? [l] : null, u, l || b(c), 32 & c.__u, d), s.__v = c.__v, s.__.__k[s.__i] = s, M(u, s, d), c.__ = c.__e = null, s.__e != l && x(s)));
	} finally {
		n.length = i = 0;
	}
}
function w(e, t, n, r, i, a, o, s, c, l, f) {
	var p, m, h, g, _, v, y = r.__k || d, b = t.length;
	for (c = T(n, t, y, c, b), p = 0; p < b; p++) (h = n.__k[p]) != null && (m = ~h.__i && y[h.__i] || u, h.__i = p, v = j(e, h, m, i, a, o, s, c, l, f), g = h.__e, (m.ref != h.ref || 8 & m.__u) && (m.ref && N(m.ref, null, h, m), h.ref && f.push(h.ref, h.__c || g, h)), _ = _ || g, 4 & h.__u ? (c = E(h, c, e, !m.__v), m.__e && (m.__e = null)) : typeof h.type == "function" && v !== void 0 ? c = v : g && (c = g.nextSibling), h.__u &= -7);
	return n.__e = _, c;
}
function T(e, t, n, r, i) {
	var a, o, s, c, l, u, d, f, m, h, g = n.length, y = g, x = 0, S = !1, C = e.__k = Array(i);
	for (a = 0; a < i; a++) (o = t[a]) != null && typeof o != "boolean" && typeof o != "function" ? (typeof o != "object" || o.constructor == String ? o = C[a] = _(null, o) : p(o) ? o = C[a] = _(v, { children: o }) : o.constructor === void 0 && o.__b ? o = C[a] = _(o.type, o.props, o.key, o.ref, o.__v) : C[a] = o, c = a + x, o.__ = e, o.__b = e.__b + 1, s = null, ~(l = o.__i = D(o, n, c, y)) && (y--, (s = n[l]) && (s.__u |= 2)), s && s.__v ? (o.__u |= 2, l == c - 1 ? x-- : l == c + 1 ? x++ : l != c && (l > c ? x-- : x++, S = !0)) : (~l || (i > g ? x-- : i < g && x++), typeof o.type != "function" && (o.__u |= 4))) : C[a] = null;
	if (S) {
		for (u = [], d = [], a = 0; a < i; a++) if ((o = C[a]) && 2 & o.__u) {
			for (f = 0, m = u.length; f < m;) u[h = f + m >> 1] < o.__i ? f = h + 1 : m = h;
			u[f] = o.__i, d[a] = f + 1;
		}
		for (x = u.length; a--;) d[a] && (d[a] == x ? x-- : C[a].__u |= 4);
	}
	if (y) for (a = 0; a < g; a++) !(s = n[a]) || 2 & s.__u || (s.__e == r && (r = b(s)), P(s, s));
	return r;
}
function E(e, t, n, r) {
	var i, a, o;
	if (typeof e.type == "function") {
		if (e.props.__P) return t;
		if (i = e.__k) for (a = 0; a < i.length; a++) i[a] && (i[a].__ = e, t = E(i[a], t, n, !1));
		return t;
	}
	for (t && !t.parentNode && (t = b(e)) && !t.parentNode && (t = null), o = t; o && o.nodeType == 8;) o = o.nextSibling;
	for (e.__e != o && (!r && n.moveBefore && e.__e.parentNode ? n.moveBefore(e.__e, t) : n.insertBefore(e.__e, t || null)), t = e.__e; (t = t && t.nextSibling) && t.nodeType == 8;);
	return t;
}
function D(e, t, n, r) {
	var i, a, o, s = e.key, c = e.type, l = t[n], u = l && !(2 & l.__u);
	if (l === null && s == null || u && s == l.key && c == l.type) return n;
	if (r > +!!u) {
		for (i = n - 1, a = n + 1; i >= 0 || a < t.length;) if ((l = t[o = i >= 0 ? i-- : a++]) && !(2 & l.__u) && s == l.key && c == l.type) return o;
	}
	return -1;
}
function O(e, t, n) {
	n ?? (n = ""), t[0] == "-" ? e.setProperty(t, n) : e[t] = n;
}
function k(e, t, n, r, i) {
	var a, o;
	n: if (t == "style") {
		if (typeof n == "string") e.style.cssText = n;
		else {
			if (typeof r == "string" && (e.style.cssText = r = ""), r) for (t in r) n && t in n || O(e.style, t, "");
			if (n) for (t in n) r && n[t] == r[t] || O(e.style, t, n[t]);
		}
	} else if (t[0] == "o" && t[1] == "n") (e.__e || (e.__e = {}))[t] = n, n && r || (o = l[t] || (l[t] = A(t)), (e.__a || (e.__a = {}))[t] = c, a = t != (t = t.replace(s, "$1")), (t = t.slice(2))[0] < "a" && (t = t.toLowerCase()), n ? e.addEventListener(t, o, a) : e.removeEventListener(t, o, a));
	else {
		if (i == "http://www.w3.org/2000/svg") t = t.replace(/xlink(H|:h)/, "h").replace(/sName$/, "s");
		else if (t != "width" && t != "height" && t != "href" && t != "list" && t != "form" && t != "tabIndex" && t != "download" && t != "rowSpan" && t != "colSpan" && t != "role" && t != "popover" && t in e) try {
			e[t] = n ?? "";
			break n;
		} catch {}
		typeof n == "function" || (n == null || !1 === n && t[4] != "-" ? e.removeAttribute(t) : e.setAttribute(t, t == "popover" && n == 1 ? "" : n));
	}
}
function A(t) {
	return function(n) {
		if (this.__e) {
			var r = this.__e[t];
			if (n[o] == null) n[o] = c++;
			else if (n[o] < this.__a[t]) return;
			return r(e.event ? e.event(n) : n);
		}
	};
}
function j(t, n, r, i, a, o, s, c, l, u) {
	var f, m, _, x, S, C, T, E, D, O, k, A, j, M, N, F, I, L, R, z, B = n.type;
	if (n.constructor !== void 0) return null;
	if (128 & r.__u && (l = 32 & r.__u, f = r.__c.__z)) {
		if (n.__u |= l, m = o = [], f.nodeType == 8) for (_ = 1, x = f.nextSibling; x; x = x.nextSibling) {
			if (x.nodeType == 8) {
				if (x.data.startsWith("$s")) _++;
				else if (x.data.startsWith("/$s") && !--_) break;
			}
			o.push(x);
		}
		else o.push(f);
		c = o[0];
	}
	(f = e.__b) && f(n);
	n: if (typeof B == "function") {
		S = s.length;
		try {
			if (O = n.props, k = (f = B.prototype) && f.render, A = (f = B.contextType) && i[f.__c], j = f ? A ? A.props.value : f.__ : i, r.__c ? 2 & (C = n.__c = r.__c).__g && (C.__g |= 1) : (k ? n.__c = C = new B(O, j) : (n.__c = C = new y(O, j), C.constructor = B, C.render = ne), A && A.sub(C), C.state || (C.state = {}), C.__n = i, C.__g |= 8, C.__h = [], C.__k = []), k && (C.__s || (C.__s = C.state), B.getDerivedStateFromProps && (C.__s == C.state && (C.__s = h({}, C.__s)), h(C.__s, B.getDerivedStateFromProps(O, C.__s)))), T = C.props, E = C.state, C.__v = n, r.__c) {
				if (k && !B.getDerivedStateFromProps && O !== T && C.componentWillReceiveProps && C.componentWillReceiveProps(O, j), n.__v == r.__v && !(8 & C.__g) || !(4 & C.__g) && C.shouldComponentUpdate && !1 === C.shouldComponentUpdate(O, C.__s, j)) {
					n.__v != r.__v && (C.props = O, C.state = C.__s, C.__g &= -9), n.__e = r.__e, n.__k = r.__k, n.__k.some(function(e) {
						e && (e.__ = n);
					}), d.push.apply(C.__h, C.__k), C.__k = [], C.__h.length && s.push(C), c = b(r);
					break n;
				}
				C.componentWillUpdate && C.componentWillUpdate(O, C.__s, j), k && C.componentDidUpdate && C.__h.push(function() {
					C.componentDidUpdate(T, E, D);
				});
			} else k && !B.getDerivedStateFromProps && C.componentWillMount && C.componentWillMount(), k && C.componentDidMount && C.__h.push(C.componentDidMount);
			if (C.context = j, C.props = O, C.__P = t, C.__g &= -5, M = e.__r, N = 0, k) C.state = C.__s, C.__g &= -9, M && M(n), f = C.render(C.props, C.state, C.context), d.push.apply(C.__h, C.__k), C.__k = [];
			else do
				C.__g &= -9, M && M(n), f = C.render(C.props, C.state, C.context), C.state = C.__s;
			while (8 & C.__g && ++N < 25);
			C.state = C.__s, C.getChildContext && (i = h({}, i, C.getChildContext())), k && r.__c && C.getSnapshotBeforeUpdate && (D = C.getSnapshotBeforeUpdate(T, E)), F = f && f.type === v && f.key == null ? f.props.children : f, O.__P && (f = c, a = (t = O.__P).namespaceURI, l = o = null, r.props && r.props.__P != t && (r.__k.some(function(e) {
				e && P(e, e);
			}), r.__k = null), c = r.__k ? b(r, 0) : null), c = w(t, p(F) ? F : [F], n, r, i, a, o, s, c, l, u), O.__P && (n.__e = null, c = f), n.__u &= -161, 128 & r.__u && (C.__z = null), m && m.some(g), C.__h.length && s.push(C), 1 & C.__g && (C.__g &= -4);
		} catch (t) {
			if (s.length = S, n.__v = null, l || o) {
				if (t.then) {
					if (I = 0, n.__u |= l ? 160 : 128, o) {
						for (~(R = o.indexOf(c || void 0)) || (R = o.length); (z = o[R - 1]) && z.nodeType == 8;) R--;
						for (; R < o.length; R++) if (z = o[R]) {
							if (o[R] = null, z.nodeType == 8) {
								if (z.data.startsWith("$s")) I++ || (L = z);
								else if (I && z.data.startsWith("/$s") && !--I) {
									c = z;
									break;
								}
							} else if (!I) break;
						}
					}
					if (!L) {
						for (; c && c.nodeType == 8 && c.nextSibling;) c = c.nextSibling;
						L = c;
					}
					n.__c.__z || (n.__c.__z = L), n.__e = c;
				} else o && o.some(g);
			} else n.__e = r.__e;
			n.__k || (n.__k = r.__k || []), t.then || ee(n), e.__e(t, n, r);
		}
	} else c = n.__e = te(r.__e, n, r, i, a, o, s, l, u, t);
	return (f = e.diffed) && f(n), 128 & n.__u ? void 0 : c;
}
function ee(e) {
	e && (e.__c && (e.__c.__g |= 4), e.__k && e.__k.some(ee));
}
function M(t, n, r) {
	for (var i = 0; i < r.length;) N(r[i++], r[i++], r[i++]);
	e.__c && e.__c(n, t), t.some(function(n) {
		try {
			t = n.__h, n.__h = [], t.some(function(e) {
				e.call(n);
			});
		} catch (t) {
			e.__e(t, n.__v);
		}
	});
}
function te(t, n, r, i, a, o, s, c, l, d) {
	var h, _, v, y, x, S, C, T, E, D = r.props || u, O = n.props, A = n.type;
	if (A == "svg" ? a = "http://www.w3.org/2000/svg" : A == "math" ? a = "http://www.w3.org/1998/Math/MathML" : a || (a = "http://www.w3.org/1999/xhtml"), o) {
		for (h = 0; h < o.length; h++) if ((x = o[h]) && (A ? x.localName == A : x.nodeType == 3)) {
			t = x, o[h] = null;
			break;
		}
	}
	if (!t) {
		if (T = d.ownerDocument || document, !A) return T.createTextNode(O);
		t = T.createElementNS(a, A, O.is && O), c && (e.__m && e.__m(n, o), c = !1), o = null;
	}
	if (A) {
		if (d = A == "template" ? t.content : t, o = A == "textarea" && O.defaultValue != null ? null : o && m.call(d.childNodes), !c && o) for (D = {}, h = 0; h < t.attributes.length; h++) D[(x = t.attributes[h]).name] = x.value;
		for (h in D) x = D[h], h == "dangerouslySetInnerHTML" ? v = x : h == "children" || h in O || h == "value" && "defaultValue" in O || h == "checked" && "defaultChecked" in O || k(t, h, null, x, a);
		for (h in E = 1 & r.__u, O) x = O[h], h == "children" ? y = x : h == "dangerouslySetInnerHTML" ? _ = x : h == "value" ? S = x : h == "checked" ? C = x : c && typeof x != "function" || !(D[h] !== x || E && x != null) || k(t, h, x, D[h], a);
		_ ? (c || v && (_.__html == v.__html || _.__html == t.innerHTML) || (t.innerHTML = _.__html), n.__k = []) : (v && (t.textContent = ""), (A == "foreignObject" || a == "http://www.w3.org/1998/Math/MathML" && f.test(A)) && (a = "http://www.w3.org/1999/xhtml"), w(d, p(y) ? y : [y], n, r, i, a, o, s, o ? o[0] : r.__k && b(r, 0), c, l), o && o.some(g)), c && A != "textarea" || (h = "value", A == "progress" && S == null ? t.removeAttribute(h) : S == null || S === t[h] && (A != "progress" || S) || k(t, h, S, D[h], a), h = "checked", C != null && C != t[h] && k(t, h, C, D[h], a));
	} else D === O || c && t.data == O || (t.data = O);
	return t;
}
function N(t, n, r, i) {
	try {
		typeof t == "function" ? n ? n.__x = t(n) || 1 : i && (i = i.__c || i.__e) && (n = i.__x) && (i.__x = null, typeof n == "function" ? n() : t(null)) : t.current = n;
	} catch (t) {
		e.__e(t, r);
	}
}
function P(t, n, r) {
	var i, a;
	if (e.unmount && e.unmount(t), !(i = t.ref) || i.current && i.current != t.__e || N(i, null, n, t), i = t.__c) {
		if (i.componentWillUnmount) try {
			i.componentWillUnmount();
		} catch (t) {
			e.__e(t, n);
		}
		i.__P = i.__n = null;
	}
	if (i = t.__k) for (a = 0; a < i.length; a++) i[a] && P(i[a], n, typeof t.type != "function" || r && !t.props.__P);
	(i = t.__e) && (r || g(i), i.__e && (i.__e = null)), t.__e = t.__c = t.__ = null;
}
function ne(e, t, n) {
	return this.constructor(e, n);
}
function F(t, n) {
	var r, i, a, o;
	e.__ && e.__(t, n), n.nodeType == 9 && (n = n.documentElement), i = (r = t && 32 & t.__u) ? null : n.__k, n.__k = _(v, { children: [t] }), a = [], o = [], j(n, n.__k, i || u, u, n.namespaceURI, i ? null : n.firstChild ? m.call(n.childNodes) : null, a, i ? i.__e : n.firstChild, r, o), M(a, n.__k, o), n.__k.props.children = null;
}
e = { __e: function(e, t, n, r) {
	for (var a, o, s; t = t.__;) if ((a = t.__c) && !(1 & a.__g)) {
		a.__g |= 4;
		try {
			if ((o = a.constructor) && o.getDerivedStateFromError && (a.setState(o.getDerivedStateFromError(e)), s = 8 & a.__g), a.componentDidCatch && (a.componentDidCatch(e, r || {}), s = 8 & a.__g), s) return void (a.__g |= 2);
		} catch (t) {
			e = t, s = 0;
		}
	}
	throw i = 0, e;
} }, t = 0, y.prototype.setState = function(e, t) {
	var n = this.__s;
	n && n != this.state || (n = this.__s = h({}, this.state)), typeof e == "function" && (e = e(h({}, n), this.props)), e && (h(n, e), this.__v && (t && this.__k.push(t), S(this)));
}, y.prototype.forceUpdate = function(e) {
	this.__v && (this.__g |= 4, e && this.__h.push(e), S(this));
}, y.prototype.render = v, n = [], i = 0, a = function(e, t) {
	return e.__v.__b - t.__v.__b;
}, o = Symbol(), s = /(PointerCapture)$|Capture$/i, c = 0, l = {};
// node_modules/preact/hooks/dist/hooks.mjs
var I, L, R, z, B = Object.is, V = 0, H = [], U = [], W = e, G = W.__b, K = W.__r, q = W.diffed, J = W.__c, Y = W.unmount, re = W.__;
function ie(e, t) {
	W.__h && W.__h(L, e, V || t), V = 0;
	var n = L.__H || (L.__H = {
		__: [],
		__h: []
	});
	return e >= n.__.length && n.__.push({}), n.__[e];
}
function X(e) {
	return V = 1, ae(fe, e);
}
function ae(e, t, n) {
	var r = ie(I++, 2);
	if (r.t = e, !r.__c && (r.__ = [n ? n(t) : fe(void 0, t), function(e) {
		var t = r.__N ? r.__N[0] : r.__[0], n = r.t(t, e);
		B(t, n) || (r.__N = [n, r.__[1]], r.__c.setState({}));
	}], r.__c = L, !L.__f)) {
		L.__f = !0;
		var i = L.shouldComponentUpdate;
		L.shouldComponentUpdate = function(e, t, n) {
			var r = this.__H;
			if (!r) return !0;
			var a = !1, o = this.props != e;
			if (r.__.some(function(e) {
				e.__N && (a = !0, B(e.__[0], e.__N[0]) || (o = !0));
			}), i) {
				var s = i.call(this, e, t, n);
				return a ? s || o : s;
			}
			return !a || o;
		};
	}
	return r.__;
}
function oe(e, t) {
	var n = ie(I++, 3);
	!W.__s && de(n.__H, t) && (n.__P = !0, n.__ = e, n.u = t, L.__H.__h.push(n));
}
function se() {
	var e;
	do {
		for (; e = U.shift();) try {
			Z(e);
		} catch (t) {
			W.__e(t, { __: (e = e.__P) && e.__v });
		}
		for (; e = H.shift();) {
			var t = e.__H;
			if (e.__P && t) try {
				t.__h.some(Z), t.__h.some(Q), t.__h = [];
			} catch (n) {
				t.__h = [], W.__e(n, e.__v);
			}
		}
	} while (U.length);
}
W.__b = function(e) {
	L = null, G && G(e);
}, W.__ = function(e, t) {
	e && t.__k && t.__k.__m && (e.__m = t.__k.__m), re && re(e, t);
}, W.__r = function(e) {
	K && K(e), I = 0;
	var t = (L = e.__c).__H;
	t && (R == L ? L.__h = [] : (t.__h.some(Z), t.__h.some(Q), I = 0), t.__h = [], t.__.some(function(e) {
		e.__N && (e.__ = e.__N), e.u = e.__N = void 0;
	})), R = L;
}, W.diffed = function(e) {
	q && q(e);
	var t = e.__c;
	t && t.__H && (t.__H.__h.length && ue(H.push(t)), t.__H.__.some(function(e) {
		e.u && (e.__H = e.u);
	})), R = L = null;
}, W.__c = function(e, t) {
	t.some(function(e) {
		try {
			e.__h.some(Z), e.__h = e.__h.filter(function(e) {
				return !e.__ || Q(e);
			});
		} catch (n) {
			t.some(function(e) {
				e.__h && (e.__h = []);
			}), t = [], W.__e(n, e.__v);
		}
	}), J && J(e, t);
}, W.unmount = function(e) {
	Y && Y(e);
	var t, n, r = e.__c;
	r && r.__H && (r.__H.__.some(function(r) {
		try {
			if (r.__P && r.__c) {
				if (n === void 0) {
					for (n = e.__; n && (!n.__c || !n.__c.__P);) n = n.__;
					n = n && n.__c;
				}
				r.__P = n, ue(U.push(r));
			} else Z(r);
		} catch (e) {
			t = e;
		}
	}), r.__H = void 0, t && W.__e(t, r.__v));
};
var ce = typeof requestAnimationFrame == "function";
function le(e) {
	var t, n = function() {
		clearTimeout(r), ce && cancelAnimationFrame(t), setTimeout(e);
	}, r = setTimeout(n, 35);
	ce && (t = requestAnimationFrame(n));
}
function ue(e) {
	e != 1 && z == W.requestAnimationFrame || ((z = W.requestAnimationFrame) || le)(se);
}
function Z(e) {
	var t = L, n = e.__c;
	typeof n == "function" && (e.__c = void 0, n()), L = t;
}
function Q(e) {
	var t = L;
	e.__c = e.__(), L = t;
}
function de(e, t) {
	return !e || e.length != t.length || t.some(function(t, n) {
		return !B(t, e[n]);
	});
}
function fe(e, t) {
	return typeof t == "function" ? t(e) : t;
}
// node_modules/preact/jsx-runtime/dist/jsxRuntime.mjs
var pe = 0;
Array.isArray;
function $(t, n, r, i, a, o) {
	n || (n = {});
	var s, c, l = n;
	if ("ref" in l && typeof t != "function") for (c in l = {}, n) c == "ref" ? s = n[c] : l[c] = n[c];
	var u = {
		type: t,
		props: l,
		key: r,
		ref: s,
		__k: null,
		__: null,
		__b: 0,
		__e: null,
		__c: null,
		constructor: void 0,
		__v: --pe,
		__i: -1,
		__u: 0
	};
	return (a || o) && (u.__source = a, u.__self = o), e.vnode && e.vnode(u), u;
}
// src/login.tsx
function me() {
	let [e, t] = X({
		controller: "DALI controller",
		version: "",
		build_utc: ""
	}), [n, r] = X(""), [i, a] = X(!1), [o, s] = X(!1), [c, l] = X("");
	oe(() => {
		let e = new AbortController();
		return fetch("/api/identity", {
			signal: e.signal,
			credentials: "same-origin"
		}).then((e) => {
			if (!e.ok) throw Error();
			return e.json();
		}).then(t).catch(() => {
			e.signal.aborted || l("Controller identity is unavailable.");
		}), () => e.abort();
	}, []);
	async function u(e) {
		if (e.preventDefault(), o || !n) return;
		s(!0), l("");
		let t = new AbortController(), i = setTimeout(() => t.abort(), 8e3);
		try {
			let e = await fetch("/api/login", {
				method: "POST",
				credentials: "same-origin",
				headers: { "Content-Type": "application/json" },
				body: JSON.stringify({ password: n }),
				signal: t.signal
			});
			if (!e.ok) {
				let t = await e.json();
				throw Error(t.error ?? "Sign-in failed.");
			}
			r(""), location.assign("/");
		} catch (e) {
			l(e.message === "The operation was aborted." ? "The controller did not answer." : e.message), s(!1);
		} finally {
			clearTimeout(i);
		}
	}
	let d = {
		id: "password",
		name: "password",
		autoComplete: "current-password",
		value: n,
		onInput: (e) => r(e.currentTarget.value),
		required: !0,
		autoFocus: !0,
		disabled: o
	};
	return /* @__PURE__ */ $("main", {
		class: "signin",
		children: [
			/* @__PURE__ */ $("div", {
				class: "brand",
				children: "DALI"
			}),
			/* @__PURE__ */ $("section", {
				class: "signin-card",
				children: [
					/* @__PURE__ */ $("p", {
						class: "eyebrow",
						children: "CONTROLLER CONFIGURATION"
					}),
					/* @__PURE__ */ $("h1", { children: e.controller }),
					/* @__PURE__ */ $("p", {
						class: "muted",
						children: "Sign in to configure buses and devices."
					}),
					/* @__PURE__ */ $("form", {
						onSubmit: u,
						children: [
							/* @__PURE__ */ $("label", {
								for: "password",
								children: "Password"
							}),
							/* @__PURE__ */ $("div", {
								class: "password",
								children: [i ? /* @__PURE__ */ $("input", {
									type: "text",
									...d
								}) : /* @__PURE__ */ $("input", {
									type: "password",
									...d
								}), /* @__PURE__ */ $("button", {
									type: "button",
									onClick: () => a(!i),
									"aria-label": i ? "Hide password" : "Show password",
									title: i ? "Hide password" : "Show password",
									"aria-controls": "password",
									children: /* @__PURE__ */ $("svg", {
										viewBox: "0 0 24 24",
										width: "20",
										height: "20",
										fill: "none",
										stroke: "currentColor",
										"stroke-width": "1.8",
										"stroke-linecap": "round",
										"stroke-linejoin": "round",
										"aria-hidden": "true",
										focusable: "false",
										children: [
											/* @__PURE__ */ $("path", { d: "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z" }),
											/* @__PURE__ */ $("circle", {
												cx: "12",
												cy: "12",
												r: "3"
											}),
											i && /* @__PURE__ */ $("path", { d: "m3 3 18 18" })
										]
									})
								})]
							}),
							/* @__PURE__ */ $("button", {
								class: "primary",
								type: "submit",
								disabled: o || !n,
								children: o ? "Signing in…" : "Sign in"
							}),
							c && /* @__PURE__ */ $("p", {
								class: "error",
								role: "alert",
								children: c
							})
						]
					}),
					/* @__PURE__ */ $("p", {
						class: "foot",
						children: "Configuration access is protected. Normal lighting control continues independently."
					})
				]
			}),
			/* @__PURE__ */ $("footer", { children: [
				"mqtt_dali ",
				e.version,
				/* @__PURE__ */ $("br", {}),
				"Build ",
				e.build_utc || "—"
			] })
		]
	});
}
F(/* @__PURE__ */ $(me, {}), document.getElementById("root"));
