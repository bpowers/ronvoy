// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use axum::http::Uri;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

use envoy_control_plane::envoy::config::route::v3::{
    route::Action as V3Action, route_action::ClusterSpecifier as V3ClusterSpecifier,
    route_match::PathSpecifier as V3PathSpecifier, Route as V3Route, RouteAction as V3RouteAction,
    RouteMatch as V3RouteMatch,
};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    MissingMatch,
    MissingAction,
    UnsupportedMatchType,
    UnsupportedClusterSpecifier,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::MissingMatch => write!(
                f,
                "route: missing match (likely mistake in creating/serializing protobuf"
            ),
            Error::MissingAction => write!(
                f,
                "route: missing action (likely mistake in creating/serializing protobuf"
            ),
            Error::UnsupportedMatchType => write!(f, "route: unsupported match type (TODO)"),
            Error::UnsupportedClusterSpecifier => {
                write!(f, "route: unsupported cluster specifier type (TODO)")
            }
        }
    }
}

impl StdError for Error {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClusterSpecifier {
    Name(String),
    // Header(String),
    // Weighted
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteAction {
    pub cluster: ClusterSpecifier,
}

impl TryFrom<V3RouteAction> for RouteAction {
    type Error = Error;

    fn try_from(value: V3RouteAction) -> Result<Self, Self::Error> {
        match value.cluster_specifier {
            Some(V3ClusterSpecifier::Cluster(name)) => Ok(RouteAction {
                cluster: ClusterSpecifier::Name(name),
            }),
            _ => Err(Error::UnsupportedClusterSpecifier),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Route(RouteAction),
    // Redirect
    // DirectResponse
}

#[derive(Debug, Clone, PartialEq)]
pub enum RouteMatch {
    Prefix(String),
    ExactPath(String),
    // SafeRegex
    // ConnectMatcher
}

impl TryFrom<V3RouteMatch> for RouteMatch {
    type Error = Error;

    fn try_from(value: V3RouteMatch) -> Result<Self, Self::Error> {
        match value.path_specifier {
            Some(V3PathSpecifier::Prefix(prefix)) => Ok(RouteMatch::Prefix(prefix)),
            Some(V3PathSpecifier::Path(prefix)) => Ok(RouteMatch::ExactPath(prefix)),
            _ => Err(Error::UnsupportedMatchType),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub name: String,
    matcher: RouteMatch,
    action: Action,
}

impl Route {
    pub fn matches(&self, uri: &Uri) -> Option<&Action> {
        match &self.matcher {
            RouteMatch::Prefix(prefix) => {
                if uri.path().starts_with(prefix) {
                    return Some(&self.action);
                }
            }
            RouteMatch::ExactPath(path) => {
                if uri.path() == path {
                    return Some(&self.action);
                }
            }
        };
        None
    }
}

impl TryFrom<V3Route> for Route {
    type Error = Error;

    fn try_from(route: V3Route) -> Result<Self, Self::Error> {
        if let Some(matcher) = route.r#match {
            if let Some(V3Action::Route(action)) = route.action {
                let matcher = RouteMatch::try_from(matcher)?;
                let action = RouteAction::try_from(action)?;
                Ok(Route {
                    name: route.name,
                    matcher,
                    action: Action::Route(action),
                })
            } else {
                Err(Error::MissingAction)
            }
        } else {
            Err(Error::MissingMatch)
        }
    }
}
